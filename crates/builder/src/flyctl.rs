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
    /// Detected Python entry point (e.g., "server.py", "main.py")
    pub python_entry: Option<String>,
    /// Whether pyproject.toml exists
    pub has_pyproject: bool,
    /// Whether uv.lock exists (indicates uv is used)
    pub has_uv_lock: bool,
    /// Whether requirements.txt exists
    pub has_requirements_txt: bool,
    /// Whether package.json exists
    pub has_package_json: bool,
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

    structure
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
CMD ["node", "index.js"]
"#)
        },
        "python" => {
            let entry = project.python_entry.as_deref().unwrap_or("main.py");
            let install_deps = generate_python_install_deps(project);
            // Optional custom build (e.g. a codegen/compile step) only when set.
            let build_step = optional_build_step(build_command);
            format!(r#"FROM python:3.11-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
{install_deps}
{build_step}EXPOSE 8000
CMD ["python", "{entry}"]
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
        "rust" => r#"FROM rust:1.75-slim AS builder
RUN apt-get update && apt-get install -y --no-install-recommends build-essential && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src
COPY . .
RUN touch src/main.rs
RUN cargo build --release
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/* .
EXPOSE 8080
CMD ["./server"]
"#.to_string(),
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
    // Priority for entry command:
    // 1. User-specified entry_command (from BuildJob)
    // 2. Entry command from existing Dockerfile
    // 3. Auto-detected based on project structure
    let effective_entry_command = entry_command
        .or(existing_entry_command)
        .map(|s| s.to_string());

    // If there's an existing Dockerfile with entry command, use multi-stage build
    // that preserves the original build process
    if let (Some(dockerfile), Some(ref entry_cmd)) = (existing_dockerfile, &effective_entry_command) {
        return generate_stdio_dockerfile_with_existing(runtime, mcp_path, entry_cmd, dockerfile);
    }

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
        "rust" => format!(r#"FROM rust:1.75-slim AS builder
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
COPY --from=builder /app/target/release/* .
COPY stdio-adapter.cjs .
ENV PORT=8080
ENV MCP_PATH="{mcp_path}"
ENV MCP_HTTP_HOST=0.0.0.0
EXPOSE 8080
CMD ["node", "stdio-adapter.cjs", "./server"]
"#),
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

    // Process each line
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Skip ENTRYPOINT and CMD from original (we'll use our own)
        if trimmed.to_uppercase().starts_with("ENTRYPOINT") ||
           trimmed.to_uppercase().starts_with("CMD") {
            continue;
        }

        // Skip EXPOSE (we'll use our own port)
        if trimmed.to_uppercase().starts_with("EXPOSE") {
            continue;
        }

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

    // Log entry command source
    if let Some(ref cmd) = job.entry_command {
        on_log(&format!("Using user-specified entry command: {}", cmd));
    } else if existing_entry_command.is_some() {
        on_log("Using entry command from existing Dockerfile");
    } else if job.runtime == "python" {
        if let Some(ref entry) = project.python_entry {
            on_log(&format!("Auto-detected Python entry point: {}", entry));
        } else {
            on_log("Warning: No Python entry point detected, defaulting to main.py");
        }
    }

    // For STDIO transport, entry_command is required for all runtimes
    // Auto-detection is unreliable and leads to confusing failures
    if job.transport == "stdio"
        && job.entry_command.is_none()
        && existing_entry_command.is_none()
    {
        let example = match job.runtime.as_str() {
            "node" => "node index.js, npx @modelcontextprotocol/server-xxx",
            "python" => "python main.py, uv run mcp-server",
            "go" => "./your-binary-name",
            "rust" => "./your-binary-name stdio",
            _ => "./your-command",
        };
        anyhow::bail!(
            "Entry command is required for stdio transport. \
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
