use anyhow::{Context, Result};
use mcp_common::AppConfig;
use mcp_queue::{BuildJob, SecretEnv};
use serde::Deserialize;
use std::path::Path;
use tokio::process::Command;

/// Sanitize log output to prevent leaking secret values
/// Replaces any occurrence of secret values with "[REDACTED]"
fn sanitize_log_output(output: &str, secrets: &[SecretEnv]) -> String {
    let mut sanitized = output.to_string();
    for secret in secrets {
        if !secret.value.is_empty() {
            sanitized = sanitized.replace(&secret.value, "[REDACTED]");
        }
    }
    sanitized
}

/// Validate secret key to prevent injection attacks
/// Keys must be valid environment variable names
fn validate_secret_key(key: &str) -> Result<()> {
    if key.is_empty() {
        anyhow::bail!("Secret key cannot be empty");
    }

    // Must start with letter or underscore
    let first_char = key.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        anyhow::bail!("Secret key must start with a letter or underscore");
    }

    // Must contain only alphanumeric characters and underscores
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        anyhow::bail!("Secret key must contain only letters, numbers, and underscores");
    }

    // Reasonable length limit
    if key.len() > 256 {
        anyhow::bail!("Secret key exceeds maximum length of 256 characters");
    }

    Ok(())
}

/// Validate secret value to prevent injection attacks
/// Returns error if value contains dangerous characters
fn validate_secret_value(value: &str) -> Result<()> {
    // Block newlines which could inject additional KEY=VALUE pairs
    if value.contains('\n') || value.contains('\r') {
        anyhow::bail!("Secret value cannot contain newline characters");
    }

    // Block null bytes which can truncate strings
    if value.contains('\0') {
        anyhow::bail!("Secret value cannot contain null bytes");
    }

    // Value length limit (flyctl has limits, and very long values could cause issues)
    if value.len() > 65536 {
        anyhow::bail!("Secret value exceeds maximum length of 65536 bytes");
    }

    Ok(())
}

const FLY_API_URL: &str = "https://api.machines.dev/v1";

/// STDIO-to-SSE adapter script content (embedded at compile time)
const STDIO_ADAPTER_JS: &str = include_str!("../assets/stdio-adapter.cjs");

/// Extract the entry command (ENTRYPOINT or CMD) from an existing Dockerfile
fn extract_dockerfile_entry_command(dockerfile_content: &str) -> Option<String> {
    let mut entrypoint: Option<String> = None;
    let mut cmd: Option<String> = None;

    for line in dockerfile_content.lines() {
        let line = line.trim();

        // Handle ENTRYPOINT
        if line.to_uppercase().starts_with("ENTRYPOINT") {
            let rest = line[10..].trim();
            if let Some(command) = parse_docker_command(rest) {
                entrypoint = Some(command);
            }
        }

        // Handle CMD
        if line.to_uppercase().starts_with("CMD") {
            let rest = line[3..].trim();
            if let Some(command) = parse_docker_command(rest) {
                cmd = Some(command);
            }
        }
    }

    // ENTRYPOINT takes precedence, then CMD
    entrypoint.or(cmd)
}

/// Parse a Docker command (JSON array or shell form)
fn parse_docker_command(s: &str) -> Option<String> {
    let s = s.trim();

    // JSON array form: ["executable", "arg1", "arg2"]
    if s.starts_with('[') && s.ends_with(']') {
        // Simple JSON parsing - extract strings from array
        let inner = &s[1..s.len()-1];
        let parts: Vec<&str> = inner
            .split(',')
            .map(|p| p.trim().trim_matches('"').trim_matches('\''))
            .filter(|p| !p.is_empty())
            .collect();
        if !parts.is_empty() {
            return Some(parts.join(" "));
        }
    }

    // Shell form: executable arg1 arg2
    if !s.is_empty() {
        return Some(s.to_string());
    }

    None
}

#[derive(Debug, Deserialize)]
struct MachineInfo {
    id: String,
    #[allow(dead_code)]
    state: String,
}

/// Result of a successful deployment
pub struct DeployResult {
    pub endpoint_url: String,
    pub machine_id: Option<String>,
}

/// Generate fly.toml content for a server
fn generate_fly_toml(app_name: &str, region: &str, runtime: &str, transport: &str) -> String {
    // For STDIO transport, always use port 8000 (STDIO adapter's port)
    let internal_port = if transport == "stdio" {
        8000
    } else {
        match runtime {
            "node" => 3000,
            "python" => 8000,
            "go" | "rust" => 8080,
            _ => 3000,
        }
    };

    format!(
        r#"app = "{app_name}"
primary_region = "{region}"

[build]

[env]
  PORT = "{internal_port}"

[http_service]
  internal_port = {internal_port}
  force_https = true
  auto_stop_machines = "stop"
  auto_start_machines = true
  min_machines_running = 0

  [http_service.concurrency]
    type = "connections"
    hard_limit = 100
    soft_limit = 80

[[vm]]
  memory = "256mb"
  cpu_kind = "shared"
  cpus = 1
"#
    )
}

/// Project structure information detected from source directory
#[derive(Debug, Default)]
pub struct ProjectStructure {
    /// Detected Python entry point file (e.g., "server.py", "main.py")
    pub python_entry: Option<String>,
    /// Console-script name from pyproject.toml `[project.scripts]` (installed on PATH).
    /// Preferred over `python_entry` when present, mirroring how pip/uv run the package.
    pub python_script: Option<String>,
    /// Whether pyproject.toml exists
    pub has_pyproject: bool,
    /// Whether uv.lock exists (indicates uv is used)
    pub has_uv_lock: bool,
    /// Whether requirements.txt exists
    pub has_requirements_txt: bool,
    /// Whether package.json exists
    pub has_package_json: bool,
    /// Auto-detected Node start command derived from package.json
    /// (`npm start` / `node <main>` / `node <bin>`). None when undetectable.
    pub node_entry: Option<String>,
    /// Binary name produced by `cargo build` (from `[[bin]]` or `[package].name`).
    /// Used to run the correct executable instead of assuming `./server`.
    pub rust_bin: Option<String>,
}

impl ProjectStructure {
    /// Best-guess startup command (full command string) for a runtime, based purely
    /// on detected project files. Returns None when nothing could be inferred — callers
    /// fall back to the user-supplied `entry_command` or the runtime's hardcoded default.
    ///
    /// This is the source of "auto-detection": it lets stdio servers deploy without an
    /// explicit entry command, and lets SSE servers stop assuming `index.js`.
    fn detected_entry(&self, runtime: &str) -> Option<String> {
        match runtime {
            "node" => self.node_entry.clone(),
            "python" => self
                .python_script
                .clone()
                .or_else(|| self.python_entry.clone().map(|f| format!("python {}", f))),
            // Go always builds to /app/server in our generated Dockerfile.
            "go" => Some("./server".to_string()),
            "rust" => self.rust_bin.clone().map(|b| format!("./{}", b)),
            _ => None,
        }
    }
}

/// Detect project structure from source directory
pub async fn detect_project_structure(source_dir: &Path) -> ProjectStructure {
    let mut structure = ProjectStructure::default();

    // Check for Python project files
    structure.has_pyproject = source_dir.join("pyproject.toml").exists();
    structure.has_uv_lock = source_dir.join("uv.lock").exists();
    structure.has_requirements_txt = source_dir.join("requirements.txt").exists();
    structure.has_package_json = source_dir.join("package.json").exists();

    // Detect Python entry point (in priority order)
    let python_entry_candidates = [
        "server.py",
        "main.py",
        "app.py",
        "__main__.py",
        "run.py",
        "index.py",
        "mcp_server.py",
    ];

    for candidate in python_entry_candidates {
        if source_dir.join(candidate).exists() {
            structure.python_entry = Some(candidate.to_string());
            break;
        }
    }

    // Also check src directory for entry points
    if structure.python_entry.is_none() {
        let src_dir = source_dir.join("src");
        if src_dir.exists() {
            for candidate in python_entry_candidates {
                if src_dir.join(candidate).exists() {
                    structure.python_entry = Some(format!("src/{}", candidate));
                    break;
                }
            }
        }
    }

    // Parse manifests for richer start-command detection (language-aware).
    // Reads are best-effort: a malformed/absent manifest simply yields None.
    if structure.has_package_json {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("package.json")) {
            structure.node_entry = parse_node_entry(&content);
        }
    }
    if structure.has_pyproject {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("pyproject.toml")) {
            structure.python_script = parse_pyproject_script(&content);
        }
    }
    if source_dir.join("Cargo.toml").exists() {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("Cargo.toml")) {
            structure.rust_bin = parse_cargo_bin(&content);
        }
    }

    structure
}

/// Derive a Node start command from `package.json` contents.
/// Priority: `scripts.start` (→ `npm start`) > `main` (→ `node <main>`) > `bin` (→ `node <path>`).
/// `scripts.start`/`main` are the conventional "run this app" entry; `bin` is the
/// fallback that covers stdio CLI packages which only declare an executable.
fn parse_node_entry(package_json: &str) -> Option<String> {
    let pkg: serde_json::Value = serde_json::from_str(package_json).ok()?;

    // 1. An explicit `start` script — let npm run exactly what the project defined.
    if pkg
        .get("scripts")
        .and_then(|s| s.get("start"))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return Some("npm start".to_string());
    }

    // 2. `main` — the package's documented entry module.
    if let Some(main) = pkg.get("main").and_then(|v| v.as_str()) {
        if !main.trim().is_empty() {
            return Some(format!("node {}", main.trim()));
        }
    }

    // 3. `bin` — a CLI executable. Prefer the entry matching the package name,
    //    otherwise the first declared binary.
    match pkg.get("bin") {
        Some(serde_json::Value::String(path)) if !path.trim().is_empty() => {
            return Some(format!("node {}", path.trim()));
        }
        Some(serde_json::Value::Object(map)) => {
            let name = pkg.get("name").and_then(|v| v.as_str());
            if let Some(n) = name {
                if let Some(path) = map.get(n).and_then(|v| v.as_str()) {
                    if !path.trim().is_empty() {
                        return Some(format!("node {}", path.trim()));
                    }
                }
            }
            for path in map.values().filter_map(|v| v.as_str()) {
                if !path.trim().is_empty() {
                    return Some(format!("node {}", path.trim()));
                }
            }
        }
        _ => {}
    }

    None
}

/// Extract the first console-script name from a pyproject.toml `[project.scripts]` table.
/// These names are installed on PATH by pip/uv, so running the bare name starts the server.
fn parse_pyproject_script(pyproject: &str) -> Option<String> {
    let val: toml::Value = toml::from_str(pyproject).ok()?;
    let scripts = val.get("project")?.get("scripts")?.as_table()?;
    scripts.keys().next().cloned()
}

/// Determine the binary name produced by `cargo build`.
/// Uses the first `[[bin]]` entry's name, falling back to `[package].name`.
fn parse_cargo_bin(cargo_toml: &str) -> Option<String> {
    let val: toml::Value = toml::from_str(cargo_toml).ok()?;

    if let Some(name) = val
        .get("bin")
        .and_then(|b| b.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("name"))
        .and_then(|n| n.as_str())
    {
        if !name.trim().is_empty() {
            return Some(name.trim().to_string());
        }
    }

    val.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Generate Dockerfile if not present
/// For STDIO transport, wraps the original command with the stdio-adapter.cjs
fn generate_dockerfile(
    runtime: &str,
    transport: &str,
    mcp_path: &str,
    existing_dockerfile: Option<&str>,
    existing_entry_command: Option<&str>,
    entry_command: Option<&str>,
    build_command: Option<&str>,
    project: &ProjectStructure,
) -> String {
    // For SSE transport, use standard Dockerfiles
    if transport != "stdio" {
        return generate_sse_dockerfile(runtime, build_command, project);
    }

    // For STDIO transport, generate Dockerfile that includes the adapter
    generate_stdio_dockerfile(runtime, mcp_path, existing_dockerfile, existing_entry_command, entry_command, build_command, project)
}

/// Build step for the Node image-build stage.
/// If the user supplied a custom `build_command`, run exactly that; otherwise run the
/// project's own `build` npm script when present (a no-op if the script is absent).
fn node_build_step(build_command: Option<&str>) -> String {
    match build_command {
        Some(cmd) if !cmd.trim().is_empty() => format!("RUN {}", cmd.trim()),
        _ => "RUN npm run build --if-present".to_string(),
    }
}

/// Optional build step for runtimes that have no sensible default build.
/// Emits a `RUN <build_command>` line only when the user supplied one, else nothing.
fn optional_build_step(build_command: Option<&str>) -> String {
    match build_command {
        Some(cmd) if !cmd.trim().is_empty() => format!("RUN {}\n", cmd.trim()),
        _ => String::new(),
    }
}

/// Generate standard Dockerfile for SSE transport
fn generate_sse_dockerfile(runtime: &str, build_command: Option<&str>, project: &ProjectStructure) -> String {
    match runtime {
        "node" => {
            // Custom build_command if set, else the project's own `build` script (no-op if absent).
            let build_step = node_build_step(build_command);
            // Auto-detected start command from package.json; fall back to the historical default.
            let node_cmd = project
                .node_entry
                .as_deref()
                .map(format_entry_command_as_args)
                .unwrap_or_else(|| r#""node", "index.js""#.to_string());
            format!(r#"FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY package*.json ./
# Install ALL deps (incl. devDependencies) so a TypeScript/bundler build can run.
RUN npm ci 2>/dev/null || npm install
COPY . .
# Run the project's own build script if it declares one (tsc/esbuild/etc.).
# Output path is the project's concern; we don't hardcode it.
{build_step}
EXPOSE 3000
CMD [{node_cmd}]
"#)
        },
        "python" => {
            let install_deps = generate_python_install_deps(project);
            // Optional custom build (e.g. a codegen/compile step) only when set.
            let build_step = optional_build_step(build_command);
            let python_cmd = python_default_cmd(project);
            format!(r#"FROM python:3.11-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
{install_deps}
{build_step}EXPOSE 8000
CMD [{python_cmd}]
"#)
        },
        "go" => r#"FROM golang:1.22 AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -o /app/server .
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/server .
EXPOSE 8080
CMD ["./server"]
"#.to_string(),
        "rust" => {
            // Copy & run the actual compiled binary (its name is the crate/bin name),
            // instead of assuming a binary literally called `server`.
            let (copy_bin, run_cmd) = rust_binary_copy_and_cmd(project);
            format!(r#"FROM rust:1.75-slim AS builder
RUN apt-get update && apt-get install -y --no-install-recommends build-essential && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {{}}" > src/main.rs
RUN cargo build --release
RUN rm -rf src
COPY . .
RUN touch src/main.rs
RUN cargo build --release
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
{copy_bin}
EXPOSE 8080
CMD [{run_cmd}]
"#)
        },
        _ => r#"FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
CMD ["./start.sh"]
"#.to_string(),
    }
}

/// Generate Python dependency installation commands based on project structure
fn generate_python_install_deps(project: &ProjectStructure) -> String {
    // Priority: uv (if uv.lock exists) > pyproject.toml > requirements.txt
    if project.has_uv_lock {
        // Use uv for faster dependency installation
        r#"RUN pip install --no-cache-dir uv && uv sync --frozen"#.to_string()
    } else if project.has_pyproject {
        // Install from pyproject.toml using pip
        r#"RUN pip install --no-cache-dir ."#.to_string()
    } else if project.has_requirements_txt {
        r#"RUN pip install --no-cache-dir -r requirements.txt"#.to_string()
    } else {
        // No dependency file found, skip installation
        "# No dependency file found".to_string()
    }
}

/// Default Python CMD arguments (already JSON-quoted, comma-joined) for a project.
/// Prefers a console script from `[project.scripts]`, else `python <entry-file>`,
/// else `python main.py`.
fn python_default_cmd(project: &ProjectStructure) -> String {
    if let Some(script) = project.python_script.as_deref() {
        format!("\"{}\"", script)
    } else {
        let entry = project.python_entry.as_deref().unwrap_or("main.py");
        format!("\"python\", \"{}\"", entry)
    }
}

/// Resolve the Rust binary `COPY` line and CMD arguments for the runtime stage.
/// When the binary name is known we copy exactly that file; otherwise we fall back
/// to copying every release artifact and running `./server` (legacy behaviour).
fn rust_binary_copy_and_cmd(project: &ProjectStructure) -> (String, String) {
    match project.rust_bin.as_deref() {
        Some(bin) => (
            format!("COPY --from=builder /app/target/release/{bin} ./{bin}"),
            format!("\"./{bin}\""),
        ),
        None => (
            "COPY --from=builder /app/target/release/* .".to_string(),
            "\"./server\"".to_string(),
        ),
    }
}

/// Generate Dockerfile for STDIO transport with adapter
/// The adapter wraps the STDIO MCP server and exposes it as HTTP/SSE
fn generate_stdio_dockerfile(
    runtime: &str,
    mcp_path: &str,
    existing_dockerfile: Option<&str>,
    existing_entry_command: Option<&str>,
    entry_command: Option<&str>,
    build_command: Option<&str>,
    project: &ProjectStructure,
) -> String {
    // An entry command the user (or their committed Dockerfile) stated *explicitly*.
    // Only an explicit command triggers wrapping the project's own Dockerfile — we
    // never wrap someone's Dockerfile around a merely-guessed command.
    let explicit_entry = entry_command
        .or(existing_entry_command)
        .map(|s| s.to_string());

    // If there's an existing Dockerfile with an explicit entry command, use a
    // multi-stage build that preserves the original build process.
    if let (Some(dockerfile), Some(ref entry_cmd)) = (existing_dockerfile, &explicit_entry) {
        return generate_stdio_dockerfile_with_existing(runtime, mcp_path, entry_cmd, dockerfile);
    }

    // Priority for entry command in the generated Dockerfiles below:
    // 1. User-specified entry_command (from BuildJob)
    // 2. Entry command from existing Dockerfile
    // 3. Auto-detected from project manifests (package.json / pyproject / Cargo.toml)
    let effective_entry_command = explicit_entry.or_else(|| project.detected_entry(runtime));

    // Otherwise use default Dockerfiles with auto-detected entry points
    match runtime {
        "node" => {
            // Check if entry command is an npx command that needs special handling
            if let Some(ref cmd) = effective_entry_command {
                if let Some((package, extra_args)) = parse_npx_command(cmd) {
                    // For npx commands:
                    // 1. Install package globally (puts binary in /usr/local/bin)
                    // 2. Use npx to run it (npx resolves binary name from package.json)
                    // This avoids hardcoding binary names which vary per package
                    let extra_args_str = if extra_args.is_empty() {
                        String::new()
                    } else {
                        format!(", {}", extra_args.iter().map(|a| format!("\"{}\"", a)).collect::<Vec<_>>().join(", "))
                    };

                    // npx packages are prebuilt; only run a build when the user explicitly set one.
                    let build_step = optional_build_step(build_command);
                    return format!(r#"FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# Install the package globally - binary goes to /usr/local/bin which is in PATH
RUN npm install -g {package}
COPY package*.json ./
RUN npm ci --only=production 2>/dev/null || npm install --only=production || true
COPY . .
{build_step}# STDIO adapter is already copied to source directory
ENV PORT=3000
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 3000
# Use npx to run the globally installed package
# npx resolves the correct binary name from package.json
CMD ["node", "stdio-adapter.cjs", "npx", "{package}"{extra_args_str}]
"#);
                }
            }

            // Determine Node entry command (non-npx case)
            let node_cmd = if let Some(ref cmd) = effective_entry_command {
                // Parse entry command and format for CMD
                format_entry_command_as_args(cmd)
            } else {
                // Default to node index.js
                r#""node", "index.js""#.to_string()
            };

            // Custom build_command if set, else the project's own `build` script (no-op if absent).
            let build_step = node_build_step(build_command);
            format!(r#"FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY package*.json ./
# Install ALL deps (incl. devDependencies) so a TypeScript/bundler build can run.
RUN npm ci 2>/dev/null || npm install
COPY . .
# Run the project's own build script if it declares one (tsc/esbuild/etc.).
# Output path is the project's concern; we don't hardcode it.
{build_step}
# STDIO adapter is already copied to source directory
ENV PORT=3000
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 3000
CMD ["node", "stdio-adapter.cjs", {node_cmd}]
"#)
        },
        "python" => {
            // Determine Python entry command
            let python_cmd = if let Some(ref cmd) = effective_entry_command {
                // Parse entry command and format for CMD
                format_python_entry_command(cmd)
            } else {
                // Auto-detect entry point
                let entry = project.python_entry.as_deref().unwrap_or("main.py");
                format!(r#""python", "{}""#, entry)
            };

            let install_deps = generate_python_install_deps(project);
            // Optional custom build (e.g. a codegen/compile step) only when set.
            let build_step = optional_build_step(build_command);

            format!(r#"FROM python:3.11-slim
# Install Node.js for the STDIO-to-SSE adapter
RUN apt-get update && apt-get install -y --no-install-recommends procps curl nodejs npm && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
{install_deps}
{build_step}ENV PORT=8000
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 8000
CMD ["node", "stdio-adapter.cjs", {python_cmd}]
"#)
        },
        "go" => format!(r#"FROM golang:1.22 AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -o /app/server .

FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/server .
COPY stdio-adapter.cjs .
ENV PORT=8080
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 8080
CMD ["node", "stdio-adapter.cjs", "./server"]
"#),
        "rust" => {
            // Copy & run the actual compiled binary instead of assuming `./server`.
            let (copy_bin, run_cmd) = rust_binary_copy_and_cmd(project);
            format!(r#"FROM rust:1.75-slim AS builder
RUN apt-get update && apt-get install -y --no-install-recommends build-essential && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {{}}" > src/main.rs
RUN cargo build --release
RUN rm -rf src
COPY . .
RUN touch src/main.rs
RUN cargo build --release

FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
{copy_bin}
COPY stdio-adapter.cjs .
ENV PORT=8080
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 8080
CMD ["node", "stdio-adapter.cjs", {run_cmd}]
"#)
        },
        _ => format!(r#"FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
ENV PORT=3000
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 3000
CMD ["node", "stdio-adapter.cjs", "./start.sh"]
"#),
    }
}

/// Format a Python entry command for use in Dockerfile CMD
/// Handles various formats: "python server.py", "uv run mcp-server", etc.
fn format_python_entry_command(cmd: &str) -> String {
    // Split command into parts and format each as a quoted string
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    parts
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Generate STDIO Dockerfile that wraps an existing Dockerfile
/// Uses multi-stage build: first stage builds using original Dockerfile,
/// second stage adds Node.js and STDIO adapter
fn generate_stdio_dockerfile_with_existing(_runtime: &str, mcp_path: &str, entry_command: &str, original_dockerfile: &str) -> String {
    // Standard port for STDIO adapter
    let port = 8000;

    // Create a multi-stage build:
    // 1. First stage: Use original Dockerfile content (renamed to "app")
    // 2. Second stage: Add Node.js and STDIO adapter on top

    // Modify the original Dockerfile to be a named stage
    let app_stage = convert_to_named_stage(original_dockerfile, "app");

    // Extract ENV lines from original Dockerfile to preserve PATH and other settings
    let env_lines = extract_env_lines(original_dockerfile);

    format!(r#"# ============================================
# Stage 1: Build using original Dockerfile
# ============================================
{app_stage}

# ============================================
# Stage 2: Add STDIO adapter with Node.js
# ============================================
FROM node:20-bookworm-slim AS runtime

# Install required tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    procps curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire filesystem from the app stage
# This preserves all installed dependencies, binaries, and environment
COPY --from=app / /

WORKDIR /app

# Preserve ENV variables from original Dockerfile (especially PATH)
{env_lines}

# Copy STDIO adapter
COPY stdio-adapter.cjs .

ENV PORT={port}
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE {port}

# Run the original entry command through the STDIO adapter
CMD ["node", "stdio-adapter.cjs", {entry_cmd_json}]
"#,
        app_stage = app_stage,
        env_lines = env_lines,
        entry_cmd_json = format_entry_command_as_args(entry_command)
    )
}

/// Extract ENV lines from a Dockerfile
/// This is used to preserve environment variables like PATH when wrapping existing Dockerfiles
fn extract_env_lines(dockerfile: &str) -> String {
    let mut env_lines = Vec::new();

    for line in dockerfile.lines() {
        let trimmed = line.trim();
        if trimmed.to_uppercase().starts_with("ENV ") {
            // Skip PORT and MCP_PATH as we set them ourselves
            let upper = trimmed.to_uppercase();
            if !upper.contains("PORT=") && !upper.contains("MCP_PATH=") {
                env_lines.push(trimmed.to_string());
            }
        }
    }

    if env_lines.is_empty() {
        // Add common paths for Python and Node.js projects as fallback
        "ENV PATH=\"/app/.venv/bin:/app/node_modules/.bin:$PATH\"".to_string()
    } else {
        env_lines.join("\n")
    }
}

/// Convert a Dockerfile to a named build stage
/// Handles both single-stage and multi-stage Dockerfiles
fn convert_to_named_stage(dockerfile: &str, stage_name: &str) -> String {
    let lines: Vec<&str> = dockerfile.lines().collect();
    let mut result = Vec::new();
    let mut first_from_found = false;
    let mut last_from_index = 0;

    // Find the last FROM instruction (for multi-stage builds)
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.to_uppercase().starts_with("FROM ") {
            last_from_index = i;
        }
    }

    // A physical line ending in `\` continues onto the next physical line as
    // part of the same logical instruction. We must honor the keep/skip
    // decision made for the instruction's first line across its continuations,
    // otherwise a continuation that happens to start with a skipped keyword
    // (e.g. the `CMD` part of `HEALTHCHECK ... \` `CMD ...`) gets dropped while
    // its leading line is kept, leaving a dangling backslash that swallows the
    // following instruction.
    #[derive(PartialEq)]
    enum Cont {
        No,
        Keep,
        Skip,
    }
    let mut cont = Cont::No;

    // Process each line
    for (i, line) in lines.iter().enumerate() {
        let continues = line.trim_end().ends_with('\\');

        // Mid-instruction continuation: stick with the prior decision.
        match cont {
            Cont::Skip => {
                cont = if continues { Cont::Skip } else { Cont::No };
                continue;
            }
            Cont::Keep => {
                result.push(line.to_string());
                cont = if continues { Cont::Keep } else { Cont::No };
                continue;
            }
            Cont::No => {}
        }

        let trimmed = line.trim();

        // Skip ENTRYPOINT and CMD from original (we'll use our own), and
        // EXPOSE (we'll use our own port) — including any continuation lines.
        if trimmed.to_uppercase().starts_with("ENTRYPOINT") ||
           trimmed.to_uppercase().starts_with("CMD") ||
           trimmed.to_uppercase().starts_with("EXPOSE") {
            cont = if continues { Cont::Skip } else { Cont::No };
            continue;
        }

        // This line is kept; remember if its instruction continues.
        cont = if continues { Cont::Keep } else { Cont::No };

        // Add stage name to the last FROM instruction
        if trimmed.to_uppercase().starts_with("FROM ") && i == last_from_index {
            // Check if it already has an AS clause
            if trimmed.to_uppercase().contains(" AS ") {
                // Replace existing AS clause with our stage name
                let parts: Vec<&str> = trimmed.splitn(2, " AS ").collect();
                if let Some(base) = parts.first() {
                    // For multi-stage, get the part before AS
                    let base_trimmed = base.split(" AS ").next().unwrap_or(base);
                    result.push(format!("{} AS {}", base_trimmed.trim(), stage_name));
                } else {
                    result.push(format!("{} AS {}", trimmed, stage_name));
                }
            } else {
                result.push(format!("{} AS {}", trimmed, stage_name));
            }
            first_from_found = true;
        } else {
            result.push(line.to_string());
        }
    }

    // If no FROM was found, this is invalid but return as-is
    if !first_from_found {
        return dockerfile.to_string();
    }

    result.join("\n")
}

/// Format entry command as JSON arguments for CMD
fn format_entry_command_as_args(cmd: &str) -> String {
    // Split command into parts and format as JSON strings
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    parts
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Check if command is an npx command and extract package info
/// Returns (package_name, extra_args) if it's an npx command
fn parse_npx_command(cmd: &str) -> Option<(String, Vec<String>)> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() || parts[0] != "npx" {
        return None;
    }

    // Find the package name (skip flags like -y, --yes, etc.)
    let mut package_idx = 1;
    while package_idx < parts.len() {
        let part = parts[package_idx];
        if part == "-y" || part == "--yes" || part == "-q" || part == "--quiet" {
            package_idx += 1;
        } else if part.starts_with('-') {
            package_idx += 1;
        } else {
            break;
        }
    }

    if package_idx >= parts.len() {
        return None;
    }

    let package = parts[package_idx].to_string();

    // Collect any extra arguments after the package name
    let extra_args: Vec<String> = parts[package_idx + 1..]
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Don't try to derive binary name - let npx resolve it from package.json
    Some((package, extra_args))
}

/// Build and deploy using flyctl CLI with remote builder
pub async fn build_and_deploy(
    config: &AppConfig,
    job: &BuildJob,
    source_dir: &Path,
    secrets: &[SecretEnv],
    on_log: impl Fn(&str),
) -> Result<DeployResult> {
    let server_id_str = job.server_id.to_string();
    let app_name = format!(
        "mcp-{}",
        server_id_str
            .split('-')
            .next()
            .unwrap_or(&server_id_str[..8.min(server_id_str.len())])
    );

    on_log(&format!("Preparing deployment for app: {}", app_name));

    // Write fly.toml
    let fly_toml_path = source_dir.join("fly.toml");
    let fly_toml_content = generate_fly_toml(&app_name, &job.region, &job.runtime, &job.transport);
    tokio::fs::write(&fly_toml_path, &fly_toml_content)
        .await
        .context("Failed to write fly.toml")?;
    on_log("Generated fly.toml");

    // Detect project structure for smarter Dockerfile generation
    let project = detect_project_structure(source_dir).await;
    on_log(&format!(
        "Detected project structure: pyproject={}, uv={}, requirements={}, python_entry={:?}",
        project.has_pyproject, project.has_uv_lock, project.has_requirements_txt, project.python_entry
    ));

    // For STDIO transport, copy the adapter script to source directory
    if job.transport == "stdio" {
        let adapter_path = source_dir.join("stdio-adapter.cjs");
        tokio::fs::write(&adapter_path, STDIO_ADAPTER_JS)
            .await
            .context("Failed to write stdio-adapter.cjs")?;
        on_log("Copied STDIO-to-SSE adapter script");
    }

    // Generate Dockerfile
    let dockerfile_path = source_dir.join("Dockerfile");
    let existing_dockerfile = if dockerfile_path.exists() {
        tokio::fs::read_to_string(&dockerfile_path).await.ok()
    } else {
        None
    };

    // For STDIO transport with existing Dockerfile, extract the entry command
    let existing_entry_command = if job.transport == "stdio" {
        if let Some(ref content) = existing_dockerfile {
            let cmd = extract_dockerfile_entry_command(content);
            if cmd.is_some() {
                on_log("Found existing Dockerfile with entry command, will preserve it");
            }
            cmd
        } else {
            None
        }
    } else {
        None
    };

    // Auto-detected startup command from project manifests (used when neither the user
    // nor an existing Dockerfile supplied one).
    let detected_entry = project.detected_entry(&job.runtime);

    // Log entry command source
    if let Some(ref cmd) = job.entry_command {
        on_log(&format!("Using user-specified entry command: {}", cmd));
    } else if existing_entry_command.is_some() {
        on_log("Using entry command from existing Dockerfile");
    } else if let Some(ref detected) = detected_entry {
        on_log(&format!("Auto-detected startup command: {}", detected));
    }

    // For STDIO transport an entry command is required, but auto-detection from project
    // manifests now satisfies it. We only bail when nothing could be inferred.
    if job.transport == "stdio"
        && job.entry_command.is_none()
        && existing_entry_command.is_none()
        && detected_entry.is_none()
    {
        let example = match job.runtime.as_str() {
            "node" => "node index.js, npx @modelcontextprotocol/server-xxx",
            "python" => "python main.py, uv run mcp-server",
            "go" => "./your-binary-name",
            "rust" => "./your-binary-name stdio",
            _ => "./your-command",
        };
        anyhow::bail!(
            "Entry command is required for stdio transport and could not be auto-detected. \
            Please set the startup command in the server settings (e.g., {}).",
            example
        );
    }

    // Generate Dockerfile if needed
    let should_generate = existing_dockerfile.is_none() || job.transport == "stdio";
    if should_generate {
        let dockerfile_content = generate_dockerfile(
            &job.runtime,
            &job.transport,
            &job.mcp_path,
            existing_dockerfile.as_deref(),
            existing_entry_command.as_deref(),
            job.entry_command.as_deref(),
            job.build_command.as_deref(),
            &project,
        );
        tokio::fs::write(&dockerfile_path, &dockerfile_content)
            .await
            .context("Failed to write Dockerfile")?;
        on_log(&format!(
            "Generated Dockerfile for {} runtime (transport: {})",
            job.runtime, job.transport
        ));
    }

    // Create app if it doesn't exist
    on_log(&format!("Creating/verifying Fly.io app: {}", app_name));
    let create_output = Command::new("flyctl")
        .args(["apps", "create", &app_name, "--org", &config.flyio.org_slug])
        .env("FLY_API_TOKEN", &config.flyio.api_token)
        .current_dir(source_dir)
        .output()
        .await
        .context("Failed to run flyctl apps create")?;

    // Ignore error if app already exists
    if !create_output.status.success() {
        let stderr = String::from_utf8_lossy(&create_output.stderr);
        if !stderr.contains("already exists") {
            tracing::warn!("App create warning: {}", stderr);
        }
    }

    // Set secrets if any
    if !secrets.is_empty() {
        // Validate all secrets before processing
        for secret in secrets {
            validate_secret_key(&secret.key)
                .with_context(|| format!("Invalid secret key: {}", secret.key))?;
            validate_secret_value(&secret.value)
                .with_context(|| format!("Invalid secret value for key: {}", secret.key))?;
        }

        on_log(&format!("Setting {} secrets...", secrets.len()));
        let mut secret_args: Vec<String> = vec!["secrets".to_string(), "set".to_string()];
        for secret in secrets {
            secret_args.push(format!("{}={}", secret.key, secret.value));
        }
        secret_args.push("--app".to_string());
        secret_args.push(app_name.clone());

        let secrets_output = Command::new("flyctl")
            .args(&secret_args)
            .env("FLY_API_TOKEN", &config.flyio.api_token)
            .current_dir(source_dir)
            .output()
            .await
            .context("Failed to set secrets")?;

        if !secrets_output.status.success() {
            let stderr = String::from_utf8_lossy(&secrets_output.stderr);
            // Sanitize stderr to prevent leaking secret values in logs
            let sanitized_stderr = sanitize_log_output(&stderr, secrets);
            tracing::warn!("Secrets warning: {}", sanitized_stderr);
        }
    }

    // Deploy with remote builder
    // Note: Region is already set in fly.toml as primary_region
    on_log("Starting remote build and deploy...");
    let deploy_output = Command::new("flyctl")
        .args([
            "deploy",
            "--remote-only",
            "--app",
            &app_name,
            "--yes",
        ])
        .env("FLY_API_TOKEN", &config.flyio.api_token)
        .current_dir(source_dir)
        .output()
        .await
        .context("Failed to run flyctl deploy")?;

    let stdout = String::from_utf8_lossy(&deploy_output.stdout);
    let stderr = String::from_utf8_lossy(&deploy_output.stderr);

    // Log output
    for line in stdout.lines() {
        if !line.trim().is_empty() {
            on_log(line);
        }
    }

    if !deploy_output.status.success() {
        // Sanitize stderr to prevent leaking secret values in logs
        let sanitized_stderr = sanitize_log_output(&stderr, secrets);
        for line in sanitized_stderr.lines() {
            if !line.trim().is_empty() {
                on_log(&format!("ERROR: {}", line));
            }
        }
        return Err(anyhow::anyhow!("Deploy failed: {}", sanitized_stderr));
    }

    on_log("Deployment successful!");
    let endpoint_url = format!("https://{}.fly.dev", app_name);

    // Get machine ID from Fly.io API
    let machine_id = get_machine_id(config, &app_name).await.ok();
    if let Some(ref id) = machine_id {
        on_log(&format!("Machine ID: {}", id));
    }

    Ok(DeployResult {
        endpoint_url,
        machine_id,
    })
}

/// Get the machine ID for an app from Fly.io API
async fn get_machine_id(config: &AppConfig, app_name: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/apps/{}/machines", FLY_API_URL, app_name))
        .header("Authorization", format!("Bearer {}", config.flyio.api_token))
        .send()
        .await
        .context("Failed to list machines")?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to list machines"));
    }

    let machines: Vec<MachineInfo> = response.json().await?;
    machines
        .into_iter()
        .next()
        .map(|m| format!("{}:{}", app_name, m.id))
        .ok_or_else(|| anyhow::anyhow!("No machines found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Node: package.json start-command detection ----

    #[test]
    fn node_prefers_start_script() {
        let pkg = r#"{"scripts": {"start": "node dist/server.js"}, "main": "index.js"}"#;
        assert_eq!(parse_node_entry(pkg).as_deref(), Some("npm start"));
    }

    #[test]
    fn node_falls_back_to_main() {
        let pkg = r#"{"main": "dist/index.js"}"#;
        assert_eq!(parse_node_entry(pkg).as_deref(), Some("node dist/index.js"));
    }

    #[test]
    fn node_bin_string() {
        let pkg = r#"{"bin": "./cli.js"}"#;
        assert_eq!(parse_node_entry(pkg).as_deref(), Some("node ./cli.js"));
    }

    #[test]
    fn node_bin_object_prefers_package_name() {
        let pkg = r#"{"name": "my-mcp", "bin": {"other": "o.js", "my-mcp": "server.js"}}"#;
        assert_eq!(parse_node_entry(pkg).as_deref(), Some("node server.js"));
    }

    #[test]
    fn node_empty_start_skipped() {
        let pkg = r#"{"scripts": {"start": "  "}, "main": "index.js"}"#;
        assert_eq!(parse_node_entry(pkg).as_deref(), Some("node index.js"));
    }

    #[test]
    fn node_no_signal_returns_none() {
        assert_eq!(parse_node_entry(r#"{"name": "x"}"#), None);
        assert_eq!(parse_node_entry("not json"), None);
    }

    // ---- Python: pyproject [project.scripts] ----

    #[test]
    fn pyproject_first_script() {
        let toml = "[project]\nname = \"x\"\n[project.scripts]\nmcp-server = \"x.cli:main\"\n";
        assert_eq!(parse_pyproject_script(toml).as_deref(), Some("mcp-server"));
    }

    #[test]
    fn pyproject_no_scripts() {
        let toml = "[project]\nname = \"x\"\n";
        assert_eq!(parse_pyproject_script(toml), None);
    }

    // ---- Rust: Cargo.toml binary name ----

    #[test]
    fn cargo_package_name() {
        let toml = "[package]\nname = \"my-server\"\nversion = \"0.1.0\"\n";
        assert_eq!(parse_cargo_bin(toml).as_deref(), Some("my-server"));
    }

    #[test]
    fn cargo_explicit_bin_wins() {
        let toml = "[package]\nname = \"crate-name\"\n[[bin]]\nname = \"actual-bin\"\npath = \"src/main.rs\"\n";
        assert_eq!(parse_cargo_bin(toml).as_deref(), Some("actual-bin"));
    }

    // ---- detected_entry() per runtime ----

    #[test]
    fn detected_entry_python_script_over_file() {
        let p = ProjectStructure {
            python_script: Some("mcp-server".into()),
            python_entry: Some("main.py".into()),
            ..Default::default()
        };
        assert_eq!(p.detected_entry("python").as_deref(), Some("mcp-server"));
    }

    #[test]
    fn detected_entry_python_file_fallback() {
        let p = ProjectStructure {
            python_entry: Some("server.py".into()),
            ..Default::default()
        };
        assert_eq!(p.detected_entry("python").as_deref(), Some("python server.py"));
    }

    #[test]
    fn detected_entry_rust_and_go() {
        let p = ProjectStructure {
            rust_bin: Some("mybin".into()),
            ..Default::default()
        };
        assert_eq!(p.detected_entry("rust").as_deref(), Some("./mybin"));
        assert_eq!(p.detected_entry("go").as_deref(), Some("./server"));
        assert_eq!(ProjectStructure::default().detected_entry("rust"), None);
        assert_eq!(ProjectStructure::default().detected_entry("unknown"), None);
    }

    #[test]
    fn rust_binary_copy_uses_detected_name() {
        let p = ProjectStructure { rust_bin: Some("srv".into()), ..Default::default() };
        let (copy, cmd) = rust_binary_copy_and_cmd(&p);
        assert_eq!(copy, "COPY --from=builder /app/target/release/srv ./srv");
        assert_eq!(cmd, "\"./srv\"");

        let (copy, cmd) = rust_binary_copy_and_cmd(&ProjectStructure::default());
        assert_eq!(copy, "COPY --from=builder /app/target/release/* .");
        assert_eq!(cmd, "\"./server\"");
    }

    // ---- convert_to_named_stage: line-continuation handling ----

    #[test]
    fn named_stage_basic_tags_last_from_and_strips_cmd_expose() {
        let df = "FROM node:20-slim\nWORKDIR /app\nEXPOSE 3000\nCMD [\"node\", \"server.js\"]\n";
        let out = convert_to_named_stage(df, "app");
        assert!(out.contains("FROM node:20-slim AS app"));
        assert!(!out.to_uppercase().contains("EXPOSE"));
        assert!(!out.to_uppercase().contains("CMD"));
    }

    #[test]
    fn named_stage_healthcheck_continuation_kept_intact() {
        // The bug: `HEALTHCHECK ... \` was kept but its `CMD ...` continuation
        // line was dropped, leaving a dangling backslash.
        let df = "FROM node:20-slim\nHEALTHCHECK --interval=30s --timeout=3s \\\n    CMD curl -f http://localhost/health || exit 1\n";
        let out = convert_to_named_stage(df, "app");
        // Both physical lines of the HEALTHCHECK must survive together.
        assert!(out.contains("HEALTHCHECK --interval=30s --timeout=3s \\"));
        assert!(out.contains("CMD curl -f http://localhost/health || exit 1"));
        // No instruction line may end with a dangling backslash.
        for line in out.lines() {
            if line.trim_end().ends_with('\\') {
                // Only valid if the *next* line exists as its continuation;
                // the last line of the output must never dangle.
                assert_ne!(line, out.lines().last().unwrap());
            }
        }
        assert!(!out.trim_end().ends_with('\\'));
    }

    #[test]
    fn named_stage_multi_continuation_keep_held_to_end() {
        // Two consecutive continuations; the middle/last physical line starts
        // with CMD but must be kept because line 1 was kept.
        let df = "FROM node:20-slim\nHEALTHCHECK --interval=30s \\\n  --timeout=3s \\\n  CMD curl -f http://localhost/health || exit 1\nRUN echo done\n";
        let out = convert_to_named_stage(df, "app");
        assert!(out.contains("HEALTHCHECK --interval=30s \\"));
        assert!(out.contains("--timeout=3s \\"));
        assert!(out.contains("CMD curl -f http://localhost/health || exit 1"));
        // The instruction *after* the multi-line HEALTHCHECK is preserved.
        assert!(out.contains("RUN echo done"));
        assert!(!out.trim_end().ends_with('\\'));
    }

    #[test]
    fn named_stage_multiline_cmd_fully_skipped() {
        // A multi-line CMD must be skipped entirely — no dangling backslash.
        let df = "FROM node:20-slim\nCMD [\"node\", \\\n    \"server.js\"]\nRUN echo after\n";
        let out = convert_to_named_stage(df, "app");
        assert!(!out.to_uppercase().contains("CMD"));
        assert!(!out.contains("server.js"));
        assert!(out.contains("RUN echo after"));
        assert!(!out.trim_end().ends_with('\\'));
    }
}
