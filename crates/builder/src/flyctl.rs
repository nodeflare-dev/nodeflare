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

// ============================================================================
// Dockerfile context-fit check (judgment B)
//
// A repo's own Dockerfile is only usable when built from a given context if all
// of its COPY/ADD sources resolve inside that context. A Dockerfile written for
// a monorepo root (e.g. `COPY src/filesystem /app`, `COPY tsconfig.json …`) cannot
// build from the `src/filesystem` subdirectory — those paths aren't there. Adopting
// it anyway breaks the build and pins a stale entry command. We detect that and
// fall back to generating our own Dockerfile instead.
//
// Direction of the check: we only discard the repo Dockerfile on a *provable*
// escape (a literal path that isn't present, or a `..` traversal). Anything we
// can't resolve statically (unresolved ${VARS}, remote URLs, globs) is left to the
// author — a false "usable" is caught loudly by the build / the post-deploy probe,
// whereas a false "unusable" would silently replace a working build.
// ============================================================================

/// A build-context-relative source path referenced by a COPY/ADD instruction.
#[derive(Debug, Clone, PartialEq)]
struct ContextSource {
    /// The source path as written (may contain globs), e.g. "src/filesystem".
    raw: String,
    /// The full logical instruction, for diagnostics, e.g. "COPY src/filesystem /app".
    instruction: String,
}

#[derive(Debug, PartialEq)]
enum EscapeReason {
    /// The path does not exist inside the build context.
    NotFound,
    /// The path reaches outside the context via `..`.
    ParentTraversal,
}

struct EscapingSource {
    instruction: String,
    source: String,
    #[allow(dead_code)]
    reason: EscapeReason,
}

enum DockerfileContextFit {
    Usable,
    Unusable { escaping: Vec<EscapingSource> },
}

/// Join physical Dockerfile lines into logical instructions, honoring `\` line
/// continuations and dropping full-line comments / blank lines. A `#` only starts
/// a comment when it is not in the middle of a continued instruction.
fn logical_instructions(dockerfile: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for raw_line in dockerfile.lines() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();
        if current.is_empty() && (trimmed.is_empty() || trimmed.starts_with('#')) {
            continue;
        }
        let continues = line.trim_end().ends_with('\\');
        let segment = if continues {
            let s = line.trim_end();
            s[..s.len() - 1].trim()
        } else {
            trimmed
        };
        if current.is_empty() {
            current.push_str(segment);
        } else if !segment.is_empty() {
            current.push(' ');
            current.push_str(segment);
        }
        if !continues {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Split the text after a COPY/ADD keyword into (leading flags, operands).
/// Handles shell form (`--flag a b dest`) and JSON exec form (`--flag ["a","dest"]`).
fn split_copy_tokens(rest: &str) -> (Vec<String>, Vec<String>) {
    let mut flags = Vec::new();
    let mut remainder = rest.trim();
    while let Some(stripped) = remainder.strip_prefix("--") {
        let end = stripped.find(char::is_whitespace).unwrap_or(stripped.len());
        flags.push(format!("--{}", &stripped[..end]));
        remainder = stripped[end..].trim_start();
    }
    let operands = if remainder.starts_with('[') {
        parse_json_string_array(remainder)
    } else {
        tokenize_shell(remainder)
    };
    (flags, operands)
}

/// Extract string elements from a JSON array literal: `["a", "b"]` -> [a, b].
fn parse_json_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !(s.starts_with('[') && s.ends_with(']')) {
        return Vec::new();
    }
    s[1..s.len() - 1]
        .split(',')
        .map(|p| p.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Quote-aware whitespace tokenizer for the shell form of COPY/ADD.
fn tokenize_shell(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut in_token = false;
    for c in s.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                    in_token = true;
                } else if c.is_whitespace() {
                    if in_token {
                        tokens.push(std::mem::take(&mut cur));
                        in_token = false;
                    }
                } else {
                    cur.push(c);
                    in_token = true;
                }
            }
        }
    }
    if in_token {
        tokens.push(cur);
    }
    tokens
}

fn has_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// True for ADD sources that come from the network / a git ref, not the context.
fn is_remote_source(src: &str) -> bool {
    let s = src.to_lowercase();
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("ftp://")
        || s.starts_with("git@")
        || s.starts_with("git://")
        || (s.contains("github.com/") && s.contains('#'))
}

/// True if the path contains an unresolved build variable (`$VAR`/`${VAR}`),
/// treating `$$` as an escaped literal dollar.
fn contains_unresolved_var(src: &str) -> bool {
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            if bytes.get(i + 1) == Some(&b'$') {
                i += 2;
                continue;
            }
            return true;
        }
        i += 1;
    }
    false
}

/// Collect every build-context-relative COPY/ADD source in a Dockerfile.
/// Excludes `--from=<stage>` copies, remote ADD URLs, heredocs and `$VAR` paths.
fn parse_context_sources(dockerfile: &str) -> Vec<ContextSource> {
    let mut sources = Vec::new();
    for instr in logical_instructions(dockerfile) {
        let mut it = instr.splitn(2, char::is_whitespace);
        let keyword = it.next().unwrap_or("").to_uppercase();
        if keyword != "COPY" && keyword != "ADD" {
            continue;
        }
        let rest = it.next().unwrap_or("").trim();
        let (flags, operands) = split_copy_tokens(rest);
        // `--from=` -> source is a build stage / external image, not the context.
        if flags.iter().any(|f| f == "--from" || f.starts_with("--from=")) {
            continue;
        }
        // Need at least one source plus a destination to interpret reliably.
        if operands.len() < 2 {
            continue;
        }
        let is_add = keyword == "ADD";
        for src in &operands[..operands.len() - 1] {
            if src.starts_with("<<") {
                continue; // heredoc inline content
            }
            if is_add && is_remote_source(src) {
                continue;
            }
            if contains_unresolved_var(src) {
                continue;
            }
            sources.push(ContextSource {
                raw: src.clone(),
                instruction: instr.clone(),
            });
        }
    }
    sources
}

/// Longest leading path with no glob metacharacter (drops the globbed segment on).
fn glob_static_prefix(norm: &str) -> String {
    let mut parts = Vec::new();
    for seg in norm.split('/') {
        if has_glob(seg) {
            break;
        }
        parts.push(seg);
    }
    parts.join("/")
}

/// Decide whether a single context source escapes `context_dir`.
fn source_escapes(
    raw: &str,
    context_dir: &Path,
    canonical_ctx: Option<&Path>,
) -> Option<EscapeReason> {
    let norm = raw.trim_start_matches('/').replace('\\', "/");
    if norm.is_empty() || norm == "." || norm == "./" {
        return None; // whole context
    }
    if norm.split('/').any(|seg| seg == "..") {
        return Some(EscapeReason::ParentTraversal);
    }
    let check_path = if has_glob(&norm) {
        glob_static_prefix(&norm)
    } else {
        norm.clone()
    };
    if check_path.is_empty() {
        return None; // glob whose parent is the context root
    }
    let full = context_dir.join(&check_path);
    if !full.exists() {
        return Some(EscapeReason::NotFound);
    }
    // Symlink safety: a link pointing outside the context is not "inside" it.
    if let (Ok(canon), Some(ctx)) = (full.canonicalize(), canonical_ctx) {
        if !canon.starts_with(ctx) {
            return Some(EscapeReason::NotFound);
        }
    }
    None
}

/// Judgment B: is the repo's Dockerfile buildable from `context_dir`?
fn dockerfile_context_fit(dockerfile: &str, context_dir: &Path) -> DockerfileContextFit {
    let canonical_ctx = context_dir.canonicalize().ok();
    let mut escaping = Vec::new();
    for cs in parse_context_sources(dockerfile) {
        if let Some(reason) = source_escapes(&cs.raw, context_dir, canonical_ctx.as_deref()) {
            escaping.push(EscapingSource {
                instruction: cs.instruction,
                source: cs.raw,
                reason,
            });
        }
    }
    if escaping.is_empty() {
        DockerfileContextFit::Usable
    } else {
        DockerfileContextFit::Unusable { escaping }
    }
}

/// Workspace member globs declared in a package.json (npm/yarn workspaces).
/// Handles both the array form and the `{ "packages": [...] }` object form.
/// Empty when the field is absent.
pub(crate) fn parse_workspaces(package_json: &str) -> Vec<String> {
    let Ok(pkg) = serde_json::from_str::<serde_json::Value>(package_json) else {
        return Vec::new();
    };
    match pkg.get("workspaces") {
        Some(serde_json::Value::Array(a)) => {
            a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
        }
        Some(serde_json::Value::Object(o)) => o
            .get("packages")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Extract package globs from a pnpm-workspace.yaml `packages:` list. Minimal and
/// line-based (avoids pulling in a YAML dependency): collects `- <glob>` items under
/// the `packages:` key (block form) or `packages: [...]` (inline flow form), strips
/// quotes, and skips negation (`!`) exclusion patterns.
pub(crate) fn parse_pnpm_workspace(yaml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_packages = false;
    for raw in yaml.lines() {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indented = line.starts_with([' ', '\t']);
        if !indented {
            // A new top-level key ends any previous block; check if it's `packages:`.
            in_packages = false;
            if let Some(rest) = trimmed.strip_prefix("packages:") {
                let rest = rest.trim();
                if rest.starts_with('[') {
                    out.extend(parse_inline_glob_array(rest));
                } else {
                    in_packages = true;
                }
            }
            continue;
        }
        if in_packages {
            if let Some(item) = trimmed.strip_prefix('-') {
                let g = item.trim().trim_matches('"').trim_matches('\'').to_string();
                if !g.is_empty() && !g.starts_with('!') {
                    out.push(g);
                }
            }
        }
    }
    out
}

fn parse_inline_glob_array(s: &str) -> Vec<String> {
    s.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|p| p.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|g| !g.is_empty() && !g.starts_with('!'))
        .collect()
}

/// Detect workspace member globs for a repo directory across npm/yarn
/// (package.json `workspaces`) and pnpm (pnpm-workspace.yaml). Empty when the repo
/// is not a workspaces monorepo.
pub(crate) fn detect_workspace_globs(dir: &Path) -> Vec<String> {
    let mut globs = Vec::new();
    if let Ok(pkg) = std::fs::read_to_string(dir.join("package.json")) {
        globs.extend(parse_workspaces(&pkg));
    }
    if let Ok(y) = std::fs::read_to_string(dir.join("pnpm-workspace.yaml"))
        .or_else(|_| std::fs::read_to_string(dir.join("pnpm-workspace.yml")))
    {
        globs.extend(parse_pnpm_workspace(&y));
    }
    globs.sort();
    globs.dedup();
    globs
}

/// Expand `prefix/*` and `prefix/**` workspace globs into the actual subdirectories
/// present, for guiding the user toward a concrete target. Literal members pass through.
fn list_workspace_members(root: &Path, globs: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for g in globs {
        if let Some(prefix) = g.strip_suffix("/**").or_else(|| g.strip_suffix("/*")) {
            if let Ok(rd) = std::fs::read_dir(root.join(prefix)) {
                for entry in rd.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            out.push(format!("{}/{}", prefix, name));
                        }
                    }
                }
            }
        } else if !has_glob(g) {
            out.push(g.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Whether `subdir` (repo-relative, e.g. "src/filesystem") is matched by one of the
/// workspace globs. Supports `prefix/**` (any depth), `prefix/*` (direct child),
/// `*` and literal forms.
pub(crate) fn subdir_is_workspace_member(subdir: &str, globs: &[String]) -> bool {
    let subdir = subdir.trim_matches('/');
    globs.iter().any(|g| {
        let g = g.trim_matches('/');
        if let Some(prefix) = g.strip_suffix("/**") {
            let prefix = prefix.trim_matches('/');
            prefix.is_empty()
                || (subdir.len() > prefix.len()
                    && subdir.starts_with(prefix)
                    && subdir.as_bytes().get(prefix.len()) == Some(&b'/'))
        } else if let Some(prefix) = g.strip_suffix("/*") {
            let prefix = prefix.trim_matches('/');
            match subdir.strip_prefix(prefix) {
                Some(rest) => {
                    let rest = rest.trim_start_matches('/');
                    !rest.is_empty() && !rest.contains('/')
                }
                None => false,
            }
        } else if g == "*" {
            !subdir.is_empty() && !subdir.contains('/')
        } else {
            g == subdir
        }
    })
}

/// Rewrite an entry command so it works when the build context is the repo ROOT but
/// the server lives in `subdir`. Handles `node <script> [args]` (the dominant TS-MCP
/// shape) by prefixing the script path, and `npm <...>` via `--prefix`. Any other
/// shape is returned unchanged (we can't safely rewrite it).
pub(crate) fn prefix_entry_with_subdir(entry: &str, subdir: &str) -> String {
    let subdir = subdir.trim_matches('/');
    if subdir.is_empty() {
        return entry.to_string();
    }
    let parts: Vec<&str> = entry.split_whitespace().collect();
    match parts.as_slice() {
        ["node", script, rest @ ..] if !script.starts_with('/') && !script.starts_with('-') => {
            let mut out = format!("node {}/{}", subdir, script);
            for r in rest {
                out.push(' ');
                out.push_str(r);
            }
            out
        }
        ["npm", rest @ ..] if !rest.is_empty() => {
            format!("npm --prefix {} {}", subdir, rest.join(" "))
        }
        _ => entry.to_string(),
    }
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
fn generate_fly_toml(app_name: &str, region: &str, runtime: &str, transport: &str, memory_mb: u64) -> String {
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
  memory = "{memory_mb}mb"
  cpu_kind = "shared"
  cpus = 1
"#
    )
}

/// Node package manager a project uses, which drives the install/build commands.
/// Only npm and pnpm are distinguished; yarn projects install fine via npm and are
/// left as Npm to avoid changing their (working) behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum NodePm {
    #[default]
    Npm,
    Pnpm,
}

impl NodePm {
    /// The `run` prefix used to invoke package scripts (`npm run` / `pnpm run`).
    fn runner(self) -> &'static str {
        match self {
            NodePm::Npm => "npm run",
            NodePm::Pnpm => "pnpm run",
        }
    }
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
    /// Dependency names from package.json (dependencies + devDependencies).
    /// Used by system-dependency provisioning to detect e.g. Playwright/Puppeteer.
    pub node_deps: Vec<String>,
    /// Dependency names from requirements.txt / pyproject.toml.
    pub python_deps: Vec<String>,
    /// Module paths from go.mod `require` entries.
    pub go_deps: Vec<String>,
    /// Detected Node package manager (npm vs pnpm), driving install/build commands.
    pub(crate) node_pm: NodePm,
}

impl ProjectStructure {
    /// Best-guess startup command (full command string) for a runtime, based purely
    /// on detected project files. Returns None when nothing could be inferred — callers
    /// fall back to the user-supplied `entry_command` or the runtime's hardcoded default.
    ///
    /// This is the source of "auto-detection": it lets stdio servers deploy without an
    /// explicit entry command, and lets SSE servers stop assuming `index.js`.
    pub(crate) fn detected_entry(&self, runtime: &str) -> Option<String> {
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
    if structure.has_package_json {
        structure.node_pm = detect_node_pm(source_dir);
    }

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

    // Dependency names — used by system-dependency provisioning. Best-effort: any
    // read/parse failure simply leaves the list empty (no provisioning, no change).
    if structure.has_package_json {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("package.json")) {
            structure.node_deps = parse_node_deps(&content);
        }
    }
    if structure.has_requirements_txt {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("requirements.txt")) {
            structure.python_deps.extend(parse_requirements_deps(&content));
        }
    }
    if structure.has_pyproject {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("pyproject.toml")) {
            structure.python_deps.extend(parse_pyproject_deps(&content));
        }
    }
    if source_dir.join("go.mod").exists() {
        if let Ok(content) = std::fs::read_to_string(source_dir.join("go.mod")) {
            structure.go_deps = parse_go_deps(&content);
        }
    }

    structure
}

/// Collect dependency names from package.json (dependencies + devDependencies +
/// optional/peer). Lower-cased. Best-effort: returns empty on parse failure.
fn parse_node_deps(package_json: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(pkg) = serde_json::from_str::<serde_json::Value>(package_json) else {
        return out;
    };
    for section in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        if let Some(map) = pkg.get(section).and_then(|v| v.as_object()) {
            for name in map.keys() {
                out.push(name.to_lowercase());
            }
        }
    }
    out
}

/// Extract package names from a requirements.txt (strip version specifiers/extras/markers).
fn parse_requirements_deps(requirements: &str) -> Vec<String> {
    requirements
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
        .filter_map(|l| {
            let name: String = l
                .chars()
                .take_while(|c| !matches!(c, '=' | '<' | '>' | '~' | '!' | ' ' | ';' | '[' | '(' ))
                .collect();
            let name = name.trim().to_lowercase();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// Extract dependency names from pyproject.toml (PEP 621 `[project].dependencies`
/// and Poetry `[tool.poetry.dependencies]`). Best-effort.
fn parse_pyproject_deps(pyproject: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(val) = toml::from_str::<toml::Value>(pyproject) else {
        return out;
    };
    // PEP 621: [project].dependencies = ["name>=1.0", ...]
    if let Some(arr) = val
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for item in arr {
            if let Some(s) = item.as_str() {
                let name: String = s
                    .chars()
                    .take_while(|c| !matches!(c, '=' | '<' | '>' | '~' | '!' | ' ' | ';' | '['))
                    .collect();
                let name = name.trim().to_lowercase();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
    }
    // Poetry: [tool.poetry.dependencies] is a table of name = "version".
    if let Some(table) = val
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_table())
    {
        for name in table.keys() {
            out.push(name.to_lowercase());
        }
    }
    out
}

/// Collect module paths referenced in go.mod `require` entries. Best-effort.
fn parse_go_deps(go_mod: &str) -> Vec<String> {
    go_mod
        .lines()
        .map(|l| l.trim())
        .filter_map(|l| {
            // Lines look like `require github.com/x/y v1.2.3` or, inside a
            // `require ( ... )` block, just `github.com/x/y v1.2.3`.
            let l = l.strip_prefix("require ").unwrap_or(l);
            let first = l.split_whitespace().next()?;
            (first.contains('/') && first.contains('.')).then(|| first.to_lowercase())
        })
        .collect()
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
// ============================================================================
// System-dependency provisioning
//
// Some MCP servers need a native binary / system library that `npm|pip install`
// alone does not provide at runtime (headless browsers, ffmpeg, image libs…).
// A set of detectors inspect the project's dependency names and emit a Provision
// describing what the image needs; it is then injected into whichever Dockerfile
// (generated or user-supplied) is being built.
//
// Design guarantees:
//   - Non-regression: if no detector matches, the Provision is empty and the
//     Dockerfile is left byte-for-byte unchanged.
//   - Version-safe: browser detectors delegate to the library's own installer
//     (`npx playwright install`), so the browser always matches the installed
//     package version — it is derived, never pinned/hardcoded.
//   - Idempotent: if the Dockerfile already provisions the dependency, injection
//     is skipped (`skip_if_contains`).
//   - Fail-safe: anything ambiguous (no FROM, etc.) results in no change.
// ============================================================================

/// What an image needs to satisfy a detected system dependency.
#[derive(Debug, Default)]
pub struct Provision {
    /// apt packages to install into the (final) image.
    pub apt_packages: Vec<String>,
    /// Commands to RUN *after* application dependencies are installed
    /// (e.g. `npx playwright install --with-deps chromium`).
    pub post_install: Vec<String>,
    /// Minimum machine memory (MB) this workload needs.
    pub min_memory_mb: Option<u64>,
    /// Notes surfaced to the user that the platform cannot auto-fix
    /// (e.g. the app must launch the browser with `--no-sandbox`).
    pub warnings: Vec<String>,
    /// If the Dockerfile already contains any of these substrings, skip the
    /// `post_install` steps (the project already provisions it itself).
    pub skip_if_contains: Vec<String>,
}

impl Provision {
    pub fn is_empty(&self) -> bool {
        self.apt_packages.is_empty() && self.post_install.is_empty() && self.min_memory_mb.is_none()
    }

    fn merge(&mut self, other: Provision) {
        for p in other.apt_packages {
            if !self.apt_packages.contains(&p) {
                self.apt_packages.push(p);
            }
        }
        self.post_install.extend(other.post_install);
        self.min_memory_mb = match (self.min_memory_mb, other.min_memory_mb) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        self.warnings.extend(other.warnings);
        for s in other.skip_if_contains {
            if !self.skip_if_contains.contains(&s) {
                self.skip_if_contains.push(s);
            }
        }
    }
}

fn deps_contain(deps: &[String], needles: &[&str]) -> bool {
    deps.iter()
        .any(|d| needles.iter().any(|n| d == n || d.contains(n)))
}

type Detector = fn(&ProjectStructure) -> Option<Provision>;

/// Ordered list of detectors. Add a new one here to support another dependency.
const DETECTORS: &[Detector] = &[
    detect_playwright_node,
    detect_puppeteer_node,
    detect_playwright_python,
    detect_selenium_python,
    detect_browser_go,
    detect_ffmpeg_node,
    detect_opencv_python,
];

/// Common headless-Chromium memory floor (Chromium ~0.5–1GB; suspend cap is 2GB).
const BROWSER_MEMORY_MB: u64 = 2048;

fn detect_playwright_node(p: &ProjectStructure) -> Option<Provision> {
    deps_contain(&p.node_deps, &["playwright", "@playwright/test", "playwright-core"]).then(|| {
        Provision {
            // Installs the browser matching the *installed* Playwright version,
            // plus its OS libraries — independent of the base image.
            post_install: vec!["npx playwright install --with-deps chromium".to_string()],
            min_memory_mb: Some(BROWSER_MEMORY_MB),
            skip_if_contains: vec!["playwright install".to_string()],
            ..Default::default()
        }
    })
}

fn detect_puppeteer_node(p: &ProjectStructure) -> Option<Provision> {
    deps_contain(&p.node_deps, &["puppeteer", "puppeteer-core"]).then(|| Provision {
        post_install: vec!["npx puppeteer browsers install chrome".to_string()],
        apt_packages: chromium_runtime_libs(),
        min_memory_mb: Some(BROWSER_MEMORY_MB),
        warnings: vec![
            "Puppeteer in a container must launch with `--no-sandbox` (and ideally \
             `--disable-dev-shm-usage`). Ensure your server passes these args."
                .to_string(),
        ],
        skip_if_contains: vec![
            "puppeteer browsers install".to_string(),
            "playwright install".to_string(),
        ],
    })
}

fn detect_playwright_python(p: &ProjectStructure) -> Option<Provision> {
    deps_contain(&p.python_deps, &["playwright"]).then(|| Provision {
        post_install: vec!["playwright install --with-deps chromium".to_string()],
        min_memory_mb: Some(BROWSER_MEMORY_MB),
        skip_if_contains: vec!["playwright install".to_string()],
        ..Default::default()
    })
}

fn detect_selenium_python(p: &ProjectStructure) -> Option<Provision> {
    deps_contain(&p.python_deps, &["selenium"]).then(|| Provision {
        // Selenium Manager (>=4.6) resolves the driver; we just provide a browser.
        apt_packages: vec!["chromium".to_string(), "chromium-driver".to_string()],
        min_memory_mb: Some(BROWSER_MEMORY_MB),
        warnings: vec![
            "Selenium in a container should launch Chrome with `--no-sandbox`."
                .to_string(),
        ],
        ..Default::default()
    })
}

fn detect_browser_go(p: &ProjectStructure) -> Option<Provision> {
    deps_contain(&p.go_deps, &["go-rod/rod", "chromedp/chromedp"]).then(|| Provision {
        apt_packages: vec!["chromium".to_string()],
        min_memory_mb: Some(BROWSER_MEMORY_MB),
        ..Default::default()
    })
}

fn detect_ffmpeg_node(p: &ProjectStructure) -> Option<Provision> {
    // fluent-ffmpeg shells out to a system ffmpeg binary.
    deps_contain(&p.node_deps, &["fluent-ffmpeg"]).then(|| Provision {
        apt_packages: vec!["ffmpeg".to_string()],
        ..Default::default()
    })
}

fn detect_opencv_python(p: &ProjectStructure) -> Option<Provision> {
    deps_contain(&p.python_deps, &["opencv-python", "opencv-contrib-python"]).then(|| Provision {
        apt_packages: vec!["libgl1".to_string(), "libglib2.0-0".to_string()],
        ..Default::default()
    })
}

/// Runtime shared libraries Chromium needs when not provided by a Playwright base image.
fn chromium_runtime_libs() -> Vec<String> {
    [
        "ca-certificates", "fonts-liberation", "libasound2", "libatk-bridge2.0-0",
        "libatk1.0-0", "libcups2", "libdbus-1-3", "libdrm2", "libgbm1", "libnspr4",
        "libnss3", "libxcomposite1", "libxdamage1", "libxfixes3", "libxkbcommon0",
        "libxrandr2", "libpango-1.0-0", "libcairo2",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Run all detectors and merge their requirements.
pub fn detect_provision(project: &ProjectStructure) -> Provision {
    let mut merged = Provision::default();
    for detect in DETECTORS {
        if let Some(p) = detect(project) {
            merged.merge(p);
        }
    }
    merged
}

/// Inject a Provision into a Dockerfile (generated or user-supplied).
///
/// Returns `Some(new_dockerfile)` if anything was added, or `None` when there is
/// nothing to do / nothing safe to do (caller then leaves the file untouched).
///   - apt packages → a `RUN apt-get install …` right after the final `FROM`.
///   - post_install → `RUN …` right before the final `CMD`/`ENTRYPOINT`.
pub fn apply_provision(dockerfile: &str, provision: &Provision) -> Option<String> {
    if provision.apt_packages.is_empty() && provision.post_install.is_empty() {
        return None;
    }

    let lines: Vec<&str> = dockerfile.lines().collect();
    let upper = |l: &str| l.trim_start().to_uppercase();

    let last_from = lines.iter().rposition(|l| upper(l).starts_with("FROM "))?;
    let last_cmd = lines.iter().rposition(|l| {
        let u = upper(l);
        u.starts_with("CMD") || u.starts_with("ENTRYPOINT")
    });

    // Only add apt packages not already mentioned anywhere in the Dockerfile.
    let apt_needed: Vec<String> = provision
        .apt_packages
        .iter()
        .filter(|pkg| !dockerfile.contains(pkg.as_str()))
        .cloned()
        .collect();
    let apt_line = (!apt_needed.is_empty()).then(|| {
        format!(
            "RUN apt-get update && apt-get install -y --no-install-recommends {} && rm -rf /var/lib/apt/lists/*",
            apt_needed.join(" ")
        )
    });

    // Skip post_install entirely if the project already provisions it.
    let already_provisions = provision
        .skip_if_contains
        .iter()
        .any(|m| dockerfile.contains(m.as_str()));
    let post_install: Vec<String> = if already_provisions {
        Vec::new()
    } else {
        provision.post_install.clone()
    };

    if apt_line.is_none() && post_install.is_empty() {
        return None;
    }

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 6);
    for (i, line) in lines.iter().enumerate() {
        // post_install goes just before the final CMD/ENTRYPOINT (deps exist by now).
        if Some(i) == last_cmd && !post_install.is_empty() {
            out.push("# nodeflare: provision system dependency (matches installed library version)".to_string());
            for cmd in &post_install {
                out.push(format!("RUN {}", cmd));
            }
        }
        out.push((*line).to_string());
        // apt packages go right after the final FROM (the runtime stage).
        if i == last_from {
            if let Some(ref a) = apt_line {
                out.push("# nodeflare: system packages".to_string());
                out.push(a.clone());
            }
        }
    }
    // No CMD/ENTRYPOINT found: append post_install as trailing build steps.
    if last_cmd.is_none() && !post_install.is_empty() {
        out.push("# nodeflare: provision system dependency".to_string());
        for cmd in &post_install {
            out.push(format!("RUN {}", cmd));
        }
    }

    Some(format!("{}\n", out.join("\n")))
}

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

/// Detect the Node package manager from lockfiles, the pnpm workspace file, or the
/// package.json `packageManager` field. Defaults to npm.
fn detect_node_pm(dir: &Path) -> NodePm {
    if dir.join("pnpm-lock.yaml").exists()
        || dir.join("pnpm-workspace.yaml").exists()
        || dir.join("pnpm-workspace.yml").exists()
    {
        return NodePm::Pnpm;
    }
    if let Ok(content) = std::fs::read_to_string(dir.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(pm) = v.get("packageManager").and_then(|p| p.as_str()) {
                if pm.trim_start().starts_with("pnpm") {
                    return NodePm::Pnpm;
                }
            }
        }
    }
    NodePm::Npm
}

/// Build step for the Node image-build stage.
/// If the user supplied a custom `build_command`, run exactly that; otherwise run the
/// project's own `build` script when present (a no-op if the script is absent).
fn node_build_step(build_command: Option<&str>, pm: NodePm) -> String {
    match build_command {
        Some(cmd) if !cmd.trim().is_empty() => format!("RUN {}", cmd.trim()),
        _ => format!("RUN {} build --if-present", pm.runner()),
    }
}

/// Dependency-install + source-copy + build section of the Node build stage.
/// npm copies manifests first for layer caching, then the source; pnpm copies the
/// whole context first because its lockfile, workspace file and member manifests are
/// all needed before `pnpm install` (which matters for workspace monorepos).
fn node_setup_section(pm: NodePm, build_command: Option<&str>) -> String {
    let build = node_build_step(build_command, pm);
    match pm {
        NodePm::Pnpm => format!(
            "COPY . .\n\
             # Install ALL deps (incl. devDependencies) so a TypeScript/bundler build can run.\n\
             RUN corepack enable && (pnpm install --frozen-lockfile 2>/dev/null || pnpm install)\n\
             # Run the project's own build script if it declares one (tsc/esbuild/etc.).\n\
             {build}"
        ),
        NodePm::Npm => format!(
            "COPY package*.json ./\n\
             # Install ALL deps (incl. devDependencies) so a TypeScript/bundler build can run.\n\
             RUN npm ci 2>/dev/null || npm install\n\
             COPY . .\n\
             # Run the project's own build script if it declares one (tsc/esbuild/etc.).\n\
             {build}"
        ),
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
            // Install/copy/build, using the detected package manager (npm or pnpm).
            let setup = node_setup_section(project.node_pm, build_command);
            // Auto-detected start command from package.json; fall back to the historical default.
            let node_cmd = project
                .node_entry
                .as_deref()
                .map(format_entry_command_as_args)
                .unwrap_or_else(|| r#""node", "index.js""#.to_string());
            format!(r#"FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
{setup}
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

            // Install/copy/build, using the detected package manager (npm or pnpm).
            let setup = node_setup_section(project.node_pm, build_command);
            format!(r#"FROM node:20-slim
RUN apt-get update && apt-get install -y --no-install-recommends procps curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
{setup}
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

    // Detect project structure for smarter Dockerfile generation
    let project = detect_project_structure(source_dir).await;
    on_log(&format!(
        "Detected project structure: pyproject={}, uv={}, requirements={}, python_entry={:?}",
        project.has_pyproject, project.has_uv_lock, project.has_requirements_txt, project.python_entry
    ));

    // System-dependency provisioning (headless browsers, ffmpeg, native libs…).
    // Empty when nothing is detected — in that case every downstream step behaves
    // exactly as before (no Dockerfile change, default memory).
    let provision = detect_provision(&project);
    if !provision.is_empty() {
        on_log(&format!(
            "Detected system dependencies: apt={:?}, post_install={:?}, min_memory={:?}MB",
            provision.apt_packages, provision.post_install, provision.min_memory_mb
        ));
        for w in &provision.warnings {
            on_log(&format!("⚠️  {}", w));
        }
    }

    // Write fly.toml (bump memory when a heavy dependency needs it; default 256MB).
    let fly_toml_path = source_dir.join("fly.toml");
    let memory_mb = provision.min_memory_mb.unwrap_or(256);
    let fly_toml_content =
        generate_fly_toml(&app_name, &job.region, &job.runtime, &job.transport, memory_mb);
    tokio::fs::write(&fly_toml_path, &fly_toml_content)
        .await
        .context("Failed to write fly.toml")?;
    on_log("Generated fly.toml");

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

    // Judgment B: only adopt the repo's Dockerfile if it can actually build from THIS
    // context. A Dockerfile written for a monorepo root (e.g. `COPY src/<pkg> /app`)
    // references paths that don't exist when built from a subdirectory; adopting it
    // would break the build and pin a stale entry command. Discard it — and, with it,
    // any entry command we'd otherwise extract from it — and generate our own instead.
    let existing_dockerfile = match existing_dockerfile {
        Some(df) => match dockerfile_context_fit(&df, source_dir) {
            DockerfileContextFit::Usable => Some(df),
            DockerfileContextFit::Unusable { escaping } => {
                for e in &escaping {
                    on_log(&format!(
                        "Ignoring repo Dockerfile: `{}` references `{}` which is not in the build context — generating a Dockerfile instead.",
                        e.instruction, e.source
                    ));
                }
                None
            }
        },
        None => None,
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
        // A workspaces monorepo root has no runnable entry of its own — the build
        // output lives under a specific member (e.g. src/filesystem/dist/index.js).
        // Don't guess: tell the user which members exist and how to target one.
        let workspaces = detect_workspace_globs(source_dir);
        if !workspaces.is_empty() {
            let members = list_workspace_members(source_dir, &workspaces);
            let target_hint = if members.is_empty() {
                workspaces.join(", ")
            } else {
                members.join(", ")
            };
            anyhow::bail!(
                "This looks like a monorepo (package.json declares workspaces: {}). \
                Nodeflare can't tell which server to run, so it won't guess. \
                Set the Root Directory to the target package — one of: {} — \
                and/or set the startup command (e.g. `node {}/dist/index.js`).",
                workspaces.join(", "),
                target_hint,
                members.first().map(String::as_str).unwrap_or("<package>"),
            );
        }

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
        // Inject system-dependency provisioning (no-op when nothing was detected).
        let dockerfile_content =
            apply_provision(&dockerfile_content, &provision).unwrap_or(dockerfile_content);
        tokio::fs::write(&dockerfile_path, &dockerfile_content)
            .await
            .context("Failed to write Dockerfile")?;
        on_log(&format!(
            "Generated Dockerfile for {} runtime (transport: {})",
            job.runtime, job.transport
        ));
    } else if !provision.is_empty() {
        // User-supplied Dockerfile is otherwise used as-is. When the project needs a
        // system dependency (e.g. a Playwright browser whose version must match the
        // installed package), patch it in before the final CMD without the user having
        // to change their repo. No change is written if nothing safe could be injected.
        if let Some(ref existing) = existing_dockerfile {
            if let Some(patched) = apply_provision(existing, &provision) {
                tokio::fs::write(&dockerfile_path, &patched)
                    .await
                    .context("Failed to write patched Dockerfile")?;
                on_log("Injected system-dependency provisioning into existing Dockerfile");
            }
        }
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

// ============================================================================
// Post-deploy MCP verification (judgment D)
//
// A built image + a started machine does NOT mean the MCP server runs: a wrong
// entry path leaves the stdio adapter listening (HTTP up) while the child process
// crash-loops, so the deploy looks "successful" but every real request 500s. We
// probe a real MCP `initialize` against the deployed endpoint and only trust the
// deploy when the child actually answers.
//
// Direction: fail the deploy only on a *proven* failure (the adapter returns 5xx
// across retries — i.e. the child is dead). Cold-start/network noise is reported as
// Inconclusive and does NOT fail the deploy, to avoid false negatives.
// ============================================================================

pub(crate) enum ProbeOutcome {
    /// The MCP server answered an initialize request.
    Verified,
    /// The server is reachable but failed (adapter 5xx) — the child isn't running.
    Broken(String),
    /// Couldn't get a definitive answer (timeouts/cold start) — don't fail on this.
    Inconclusive(String),
}

/// Char-safe short preview of a response body for logs/errors.
fn snippet(s: &str) -> String {
    let s = s.trim();
    let short: String = s.chars().take(200).collect();
    if short.len() < s.len() {
        format!("{}…", short)
    } else {
        short
    }
}

/// True if `body` looks like a JSON-RPC reply (json, or an SSE `data:` line).
fn looks_like_jsonrpc(body: &str) -> bool {
    let json_part = body
        .lines()
        .find_map(|l| l.trim().strip_prefix("data:").map(str::trim))
        .unwrap_or_else(|| body.trim());
    serde_json::from_str::<serde_json::Value>(json_part)
        .map(|v| v.get("result").is_some() || v.get("error").is_some() || v.get("jsonrpc").is_some())
        .unwrap_or(false)
}

/// Probe the deployed MCP server with a real `initialize` request (judgment D).
pub(crate) async fn verify_mcp_initialize(
    endpoint_url: &str,
    mcp_path: &str,
    on_log: &impl Fn(&str),
) -> ProbeOutcome {
    let path = if mcp_path.is_empty() { "/mcp" } else { mcp_path };
    let url = format!(
        "{}/{}",
        endpoint_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "nodeflare-probe", "version": "1.0" }
        }
    });

    const ATTEMPTS: usize = 8;
    let mut saw_server_error = false;
    let mut last_detail = String::from("no response");

    for attempt in 1..=ATTEMPTS {
        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() && looks_like_jsonrpc(&text) {
                    on_log(&format!("MCP server verified: initialize responded ({})", status));
                    return ProbeOutcome::Verified;
                }
                if status.is_server_error() {
                    saw_server_error = true;
                }
                last_detail = format!("HTTP {}: {}", status.as_u16(), snippet(&text));
            }
            Err(e) => {
                last_detail = format!("request error: {}", e);
            }
        }
        if attempt < ATTEMPTS {
            on_log(&format!(
                "Verifying MCP server… not ready yet (attempt {}/{}: {})",
                attempt, ATTEMPTS, last_detail
            ));
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    if saw_server_error {
        ProbeOutcome::Broken(last_detail)
    } else {
        ProbeOutcome::Inconclusive(last_detail)
    }
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

    // ---- System-dependency provisioning ----

    #[test]
    fn provision_detects_playwright_node() {
        let project = ProjectStructure {
            node_deps: vec!["playwright".to_string(), "express".to_string()],
            ..Default::default()
        };
        let p = detect_provision(&project);
        assert!(!p.is_empty());
        assert_eq!(p.min_memory_mb, Some(2048));
        assert!(p
            .post_install
            .iter()
            .any(|c| c.contains("playwright install")));
    }

    #[test]
    fn provision_empty_for_plain_api_server() {
        // No browser / native deps → nothing provisioned → builds unchanged.
        let project = ProjectStructure {
            node_deps: vec!["express".to_string(), "axios".to_string()],
            ..Default::default()
        };
        assert!(detect_provision(&project).is_empty());
    }

    #[test]
    fn apply_provision_noop_when_empty() {
        let df = "FROM node:20-slim\nCMD [\"node\", \"index.js\"]\n";
        assert_eq!(apply_provision(df, &Provision::default()), None);
    }

    #[test]
    fn apply_provision_inserts_post_install_before_cmd() {
        let project = ProjectStructure {
            node_deps: vec!["playwright".to_string()],
            ..Default::default()
        };
        let p = detect_provision(&project);
        let df = "FROM node:20-slim\nWORKDIR /app\nCOPY . .\nRUN npm ci\nCMD [\"node\", \"index.js\"]\n";
        let out = apply_provision(df, &p).expect("should inject");
        assert!(out.contains("RUN npx playwright install --with-deps chromium"));
        let run_at = out.find("playwright install").unwrap();
        let cmd_at = out.find("CMD").unwrap();
        assert!(run_at < cmd_at, "install must come before CMD");
    }

    #[test]
    fn apply_provision_idempotent_when_already_provisioned() {
        let project = ProjectStructure {
            node_deps: vec!["playwright".to_string()],
            ..Default::default()
        };
        let p = detect_provision(&project);
        // Dockerfile already runs the installer → nothing to add → no change.
        let df = "FROM node:20-slim\nRUN npm ci\nRUN npx playwright install --with-deps chromium\nCMD [\"node\", \"index.js\"]\n";
        assert_eq!(apply_provision(df, &p), None);
    }

    #[test]
    fn apply_provision_patches_existing_user_dockerfile() {
        // The note-server case: own Dockerfile, no explicit browser install.
        let project = ProjectStructure {
            node_deps: vec!["playwright".to_string()],
            ..Default::default()
        };
        let p = detect_provision(&project);
        let df = "FROM mcr.microsoft.com/playwright:v1.40.0-jammy\nWORKDIR /app\nCOPY . .\nRUN npm ci && npm run build\nEXPOSE 3000\nCMD [\"npm\", \"run\", \"start:http\"]\n";
        let out = apply_provision(df, &p).expect("should patch");
        assert!(out.contains("RUN npx playwright install --with-deps chromium"));
        assert!(out.find("playwright install").unwrap() < out.find("CMD").unwrap());
    }

    #[test]
    fn apply_provision_adds_apt_after_final_from() {
        let project = ProjectStructure {
            python_deps: vec!["selenium".to_string()],
            ..Default::default()
        };
        let p = detect_provision(&project);
        let df = "FROM python:3.11-slim\nWORKDIR /app\nCOPY . .\nRUN pip install -r requirements.txt\nCMD [\"python\", \"main.py\"]\n";
        let out = apply_provision(df, &p).expect("should inject apt");
        assert!(out.contains("apt-get install"));
        assert!(out.contains("chromium"));
    }

    // ---- judgment B: COPY/ADD context-source parsing (pure) ----

    fn raws(df: &str) -> Vec<String> {
        parse_context_sources(df).into_iter().map(|c| c.raw).collect()
    }

    #[test]
    fn ctx_sources_literal_and_multi() {
        assert_eq!(raws("COPY src/filesystem /app"), vec!["src/filesystem"]);
        assert_eq!(raws("COPY a b /dst"), vec!["a", "b"]);
    }

    #[test]
    fn ctx_sources_json_exec_form() {
        assert_eq!(raws(r#"COPY ["x", "y"]"#), vec!["x"]);
    }

    #[test]
    fn ctx_sources_flags_and_from_and_remote() {
        assert_eq!(raws("COPY --chown=node:node app /app"), vec!["app"]);
        assert!(raws("COPY --from=builder /a /b").is_empty());
        assert!(raws("COPY --from=nginx:latest /a /b").is_empty());
        assert!(raws("ADD https://h/x.tar /x").is_empty());
        assert!(raws("ADD git@github.com:o/r.git#main /x").is_empty());
    }

    #[test]
    fn ctx_sources_vars_heredoc_and_dot() {
        assert!(raws("COPY ${SRC} /app").is_empty());
        assert_eq!(raws("COPY $$literal /app"), vec!["$$literal"]);
        assert_eq!(raws("COPY . ."), vec!["."]);
    }

    #[test]
    fn ctx_sources_line_continuation_and_comments() {
        let df = "# comment\nCOPY \\\n  a \\\n  b /dst\nRUN echo hi\n";
        assert_eq!(raws(df), vec!["a", "b"]);
    }

    #[test]
    fn ctx_sources_ignores_non_copy() {
        assert!(raws("RUN npm ci\nWORKDIR /app\nFROM node:20").is_empty());
    }

    #[test]
    fn ctx_sources_glob_preserved() {
        assert_eq!(raws("COPY package*.json ./"), vec!["package*.json"]);
    }

    #[test]
    fn parse_workspaces_array_and_object() {
        assert_eq!(parse_workspaces(r#"{"workspaces":["src/*"]}"#), vec!["src/*"]);
        assert_eq!(
            parse_workspaces(r#"{"workspaces":{"packages":["a","b/*"]}}"#),
            vec!["a", "b/*"]
        );
        assert!(parse_workspaces(r#"{"name":"x"}"#).is_empty());
    }

    // ---- judgment B: context fit against a real directory tree ----

    fn write(dir: &std::path::Path, rel: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "x").unwrap();
    }

    #[test]
    fn fit_usable_when_all_sources_present() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "package.json");
        write(tmp.path(), "src/index.ts");
        let df = "FROM node:20\nWORKDIR /app\nCOPY package.json ./\nCOPY src ./src\nRUN npm ci";
        assert!(matches!(
            dockerfile_context_fit(df, tmp.path()),
            DockerfileContextFit::Usable
        ));
    }

    #[test]
    fn fit_unusable_monorepo_root_dockerfile_in_subdir() {
        // Simulates building modelcontextprotocol/servers' src/filesystem/Dockerfile
        // from the src/filesystem subdirectory: `COPY src/filesystem /app` escapes.
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "package.json");
        write(tmp.path(), "index.ts");
        let df = "FROM node:22-alpine AS builder\nWORKDIR /app\nCOPY src/filesystem /app\nCOPY tsconfig.json /tsconfig.json\nRUN npm install\nFROM node:22-alpine\nCOPY --from=builder /app/dist /app/dist\nENTRYPOINT [\"node\", \"/app/dist/index.js\"]";
        match dockerfile_context_fit(df, tmp.path()) {
            DockerfileContextFit::Unusable { escaping } => {
                let srcs: Vec<_> = escaping.iter().map(|e| e.source.as_str()).collect();
                assert!(srcs.contains(&"src/filesystem"));
                // The --from=builder copy must NOT be flagged.
                assert!(!srcs.iter().any(|s| s.contains("dist")));
            }
            DockerfileContextFit::Usable => panic!("should be unusable"),
        }
    }

    #[test]
    fn fit_flags_parent_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let df = "FROM node:20\nCOPY ../shared /app";
        match dockerfile_context_fit(df, tmp.path()) {
            DockerfileContextFit::Unusable { escaping } => {
                assert_eq!(escaping[0].reason, EscapeReason::ParentTraversal);
            }
            _ => panic!("should be unusable"),
        }
    }

    #[test]
    fn fit_glob_ok_when_parent_present() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "package.json");
        let df = "FROM node:20\nCOPY package*.json ./";
        assert!(matches!(
            dockerfile_context_fit(df, tmp.path()),
            DockerfileContextFit::Usable
        ));
    }

    #[test]
    fn list_workspace_members_expands_star() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/filesystem")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/git")).unwrap();
        let members = list_workspace_members(tmp.path(), &["src/*".to_string()]);
        assert_eq!(members, vec!["src/filesystem", "src/git"]);
    }

    // ---- strategy 3: workspace membership + entry prefixing ----

    #[test]
    fn subdir_membership() {
        let ws = vec!["src/*".to_string()];
        assert!(subdir_is_workspace_member("src/filesystem", &ws));
        assert!(subdir_is_workspace_member("/src/filesystem/", &ws));
        assert!(!subdir_is_workspace_member("src/filesystem/sub", &ws));
        assert!(!subdir_is_workspace_member("packages/a", &ws));
        assert!(subdir_is_workspace_member("a", &["*".to_string()]));
        assert!(subdir_is_workspace_member("packages/a", &["packages/a".to_string()]));
        // pnpm recursive glob: any depth under the prefix.
        let rec = vec!["packages/**".to_string()];
        assert!(subdir_is_workspace_member("packages/a", &rec));
        assert!(subdir_is_workspace_member("packages/group/a", &rec));
        assert!(!subdir_is_workspace_member("packages", &rec));
        assert!(!subdir_is_workspace_member("apps/a", &rec));
    }

    // ---- pnpm: workspace parsing + package-manager detection ----

    #[test]
    fn parse_pnpm_workspace_block_inline_and_negation() {
        let block = "packages:\n  - 'packages/*'\n  - \"apps/*\"\n  - '!**/test/**'\nonlyBuiltDependencies:\n  - esbuild\n";
        assert_eq!(parse_pnpm_workspace(block), vec!["packages/*", "apps/*"]);
        let inline = "packages: ['packages/*', 'tools/*']\n";
        assert_eq!(parse_pnpm_workspace(inline), vec!["packages/*", "tools/*"]);
        assert!(parse_pnpm_workspace("name: x\n").is_empty());
    }

    #[test]
    fn detect_workspace_globs_merges_npm_and_pnpm() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), r#"{"name":"x"}"#).unwrap();
        std::fs::write(tmp.path().join("pnpm-workspace.yaml"), "packages:\n  - 'src/*'\n").unwrap();
        assert_eq!(detect_workspace_globs(tmp.path()), vec!["src/*"]);
    }

    #[test]
    fn detect_node_pm_signals() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), r#"{"name":"x"}"#).unwrap();
        assert_eq!(detect_node_pm(tmp.path()), NodePm::Npm);
        std::fs::write(tmp.path().join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
        assert_eq!(detect_node_pm(tmp.path()), NodePm::Pnpm);

        let tmp2 = tempfile::tempdir().unwrap();
        std::fs::write(tmp2.path().join("package.json"), r#"{"packageManager":"pnpm@9.1.0"}"#).unwrap();
        assert_eq!(detect_node_pm(tmp2.path()), NodePm::Pnpm);
    }

    #[test]
    fn node_setup_section_switches_on_pm() {
        let npm = node_setup_section(NodePm::Npm, None);
        assert!(npm.contains("npm ci"));
        assert!(npm.contains("npm run build --if-present"));
        assert!(!npm.contains("pnpm"));

        let pnpm = node_setup_section(NodePm::Pnpm, None);
        assert!(pnpm.contains("corepack enable"));
        assert!(pnpm.contains("pnpm install"));
        assert!(pnpm.contains("pnpm run build --if-present"));
        assert!(!pnpm.contains("npm ci"));

        // A custom build command overrides the default for either manager.
        assert!(node_setup_section(NodePm::Pnpm, Some("pnpm compile")).contains("RUN pnpm compile"));
    }

    #[test]
    fn jsonrpc_detection() {
        assert!(looks_like_jsonrpc(r#"{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{}}}"#));
        assert!(looks_like_jsonrpc(r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32000}}"#));
        // SSE-framed reply.
        assert!(looks_like_jsonrpc("event: message\ndata: {\"jsonrpc\":\"2.0\",\"result\":{}}\n\n"));
        // Not JSON-RPC.
        assert!(!looks_like_jsonrpc("Internal Server Error"));
        assert!(!looks_like_jsonrpc("<html>502</html>"));
    }

    #[test]
    fn snippet_is_char_safe() {
        let s = "あ".repeat(300);
        let out = snippet(&s);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 201);
    }

    #[test]
    fn prefix_entry_node_and_npm() {
        assert_eq!(
            prefix_entry_with_subdir("node dist/index.js", "src/filesystem"),
            "node src/filesystem/dist/index.js"
        );
        assert_eq!(
            prefix_entry_with_subdir("node dist/index.js /tmp", "src/filesystem"),
            "node src/filesystem/dist/index.js /tmp"
        );
        assert_eq!(
            prefix_entry_with_subdir("npm start", "src/git"),
            "npm --prefix src/git start"
        );
        // Unrewritable shapes pass through unchanged.
        assert_eq!(prefix_entry_with_subdir("./server", "src/x"), "./server");
        assert_eq!(prefix_entry_with_subdir("node dist/index.js", ""), "node dist/index.js");
    }
}
