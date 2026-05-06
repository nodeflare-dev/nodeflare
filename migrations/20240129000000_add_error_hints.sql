-- Error hints table for user-friendly deployment failure messages
CREATE TABLE error_hints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Keywords to match (all must be present, case-insensitive)
    keywords TEXT[] NOT NULL,
    -- The hint message to display
    hint_message TEXT NOT NULL,
    -- Priority (higher wins when multiple patterns match)
    priority INTEGER NOT NULL DEFAULT 0,
    -- Category for organization (git, npm, python, docker, etc.)
    category VARCHAR(50) NOT NULL DEFAULT 'general',
    -- Enable/disable individual hints
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for efficient querying
CREATE INDEX idx_error_hints_active ON error_hints(is_active) WHERE is_active = true;
CREATE INDEX idx_error_hints_category ON error_hints(category);

-- Seed initial error hints (migrated from hardcoded patterns)

-- Git-related errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['remote branch', 'not found'],
'💡 Hint: The specified branch does not exist in the repository.
Please check your MCP server settings and verify the branch name.
Common branch names: ''main'', ''master'', ''develop''', 100, 'git'),

(ARRAY['repository not found'],
'💡 Hint: The repository could not be found.
- Check if the repository URL is correct
- For private repos, ensure GitHub App is installed on the repository
- Verify the repository exists and is accessible', 90, 'git'),

(ARRAY['authentication failed'],
'💡 Hint: Authentication to the repository failed.
- For private repos, reinstall the GitHub App on your repository
- Check if the repository permissions are correctly configured', 90, 'git'),

(ARRAY['could not read from remote'],
'💡 Hint: Could not read from remote repository.
- For private repos, reinstall the GitHub App on your repository
- Check if the repository permissions are correctly configured', 85, 'git');

-- Dockerfile/Build errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['dockerfile', 'not found'],
'💡 Hint: Dockerfile was not found in the repository.
- Ensure a Dockerfile exists in the root directory (or specified root_directory)
- Check if the file is named exactly ''Dockerfile'' (case-sensitive)', 80, 'docker'),

(ARRAY['no such file or directory'],
'💡 Hint: A required file or directory was not found.
- Check if the root_directory setting is correct
- Verify all required files are committed to the repository', 70, 'docker');

-- npm/Node.js errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['postinstall'],
'💡 Hint: A postinstall script failed during npm install.
This is likely because a devDependency (like simple-git-hooks, husky)
is used in the postinstall script but not installed in production.

Fix in your package.json:
Change: "postinstall": "simple-git-hooks"
To:     "postinstall": "simple-git-hooks || true"
Or:     "postinstall": "npx simple-git-hooks || true"

These tools are for local development and not needed in production.', 95, 'npm'),

(ARRAY['simple-git-hooks'],
'💡 Hint: simple-git-hooks postinstall script failed.
This tool is for local development only.

Fix in your package.json:
Change: "postinstall": "simple-git-hooks"
To:     "postinstall": "simple-git-hooks || true"', 96, 'npm'),

(ARRAY['husky'],
'💡 Hint: Husky postinstall/prepare script failed.
This tool is for local development only.

Fix in your package.json:
Change: "prepare": "husky install"
To:     "prepare": "husky install || true"', 96, 'npm'),

(ARRAY['lefthook'],
'💡 Hint: Lefthook postinstall script failed.
This tool is for local development only.

Fix in your package.json:
Change: "postinstall": "lefthook install"
To:     "postinstall": "lefthook install || true"', 96, 'npm'),

(ARRAY['npm err'],
'💡 Hint: npm encountered an error during installation.
- Check if package.json is valid
- Verify all dependencies are correctly specified
- Check if there are any private packages that require authentication
- If using postinstall scripts, ensure they work in production (devDependencies are not installed)', 50, 'npm'),

(ARRAY['npm error'],
'💡 Hint: npm encountered an error during installation.
- Check if package.json is valid
- Verify all dependencies are correctly specified
- Check if there are any private packages that require authentication', 50, 'npm'),

(ARRAY['cannot find module'],
'💡 Hint: A required Node.js module was not found.
- Check if the module is listed in your dependencies (not devDependencies)
- Verify package.json includes the module
- Check for typos in import statements', 75, 'npm');

-- Python errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['pip', 'error'],
'💡 Hint: pip encountered an error during installation.
- Check if requirements.txt or pyproject.toml is valid
- Verify all dependencies are available on PyPI
- Check for version conflicts', 60, 'python'),

(ARRAY['pip', 'failed'],
'💡 Hint: pip installation failed.
- Check if requirements.txt or pyproject.toml is valid
- Verify all dependencies are available on PyPI
- Check for version conflicts', 60, 'python'),

(ARRAY['uv', 'error'],
'💡 Hint: uv (Python package manager) encountered an error.
- Check if pyproject.toml is valid
- Verify all dependencies are available
- Check for version conflicts in your dependencies', 60, 'python'),

(ARRAY['uv', 'failed'],
'💡 Hint: uv (Python package manager) failed.
- Check if pyproject.toml is valid
- Verify all dependencies are available
- Check for version conflicts in your dependencies', 60, 'python'),

(ARRAY['modulenotfounderror'],
'💡 Hint: A required Python module was not found.
- Check if the module is listed in requirements.txt or pyproject.toml
- Verify the module name is spelled correctly
- Check for typos in import statements', 75, 'python');

-- Rust/Cargo errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['cargo', 'error'],
'💡 Hint: Cargo (Rust) build failed.
- Check if Cargo.toml is valid
- Verify all dependencies compile correctly
- Check for compilation errors in your code', 60, 'rust');

-- TypeScript errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['typescript'],
'💡 Hint: TypeScript compilation failed.
- Check for type errors in your code
- Verify tsconfig.json is correctly configured
- Ensure all type dependencies (@types/*) are installed', 55, 'typescript'),

(ARRAY['tsc'],
'💡 Hint: TypeScript compiler (tsc) failed.
- Check for type errors in your code
- Verify tsconfig.json is correctly configured', 55, 'typescript');

-- Syntax errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['syntaxerror'],
'💡 Hint: Syntax error detected in your code.
- Check the error message for the file and line number
- Common causes: missing brackets, invalid JSON, typos
- Ensure your code is valid for the language/runtime version', 70, 'syntax'),

(ARRAY['syntax error'],
'💡 Hint: Syntax error detected in your code.
- Check the error message for the file and line number
- Common causes: missing brackets, invalid JSON, typos', 70, 'syntax');

-- ES Module errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['unexpected token', 'export'],
'💡 Hint: ES Module import/export error.
- For Node.js: add "type": "module" to package.json, or use .mjs extension
- Or change exports to module.exports for CommonJS
- Check your package.json "type" field matches your code style', 80, 'esmodule'),

(ARRAY['cannot use import statement'],
'💡 Hint: ES Module import statement error.
- For Node.js: add "type": "module" to package.json, or use .mjs extension
- Or change imports to require() syntax for CommonJS
- Check your package.json "type" field matches your code style', 80, 'esmodule');

-- JSON errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['json', 'parse'],
'💡 Hint: JSON parsing error.
- Check package.json or other JSON files for syntax errors
- Common issues: trailing commas, missing quotes, invalid characters
- Use a JSON validator to check your files', 65, 'json'),

(ARRAY['json', 'unexpected token'],
'💡 Hint: JSON parsing error with unexpected token.
- Check package.json or other JSON files for syntax errors
- Common issues: trailing commas, missing quotes, invalid characters', 65, 'json');

-- Port/Network errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['port', 'already in use'],
'💡 Hint: Port conflict detected.
- The application might be trying to use a port that''s already in use
- Check your application''s port configuration', 70, 'network'),

(ARRAY['address already'],
'💡 Hint: Address already in use.
- Another process is using the same port
- Check your application''s port configuration', 70, 'network');

-- Memory/Resource errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['out of memory'],
'💡 Hint: The build ran out of memory.
- Try reducing the size of dependencies
- Consider optimizing your build process
- Contact support if this persists', 80, 'resource'),

(ARRAY['oom'],
'💡 Hint: Out of memory (OOM) error.
- Try reducing the size of dependencies
- Consider optimizing your build process', 80, 'resource'),

(ARRAY['killed'],
'💡 Hint: Process was killed (possibly OOM).
- The build might have run out of memory
- Try reducing the size of dependencies', 60, 'resource');

-- Timeout errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['timeout'],
'💡 Hint: The operation timed out.
- The build or deployment took too long
- Try simplifying your build process
- Check for slow network operations during build', 65, 'timeout'),

(ARRAY['timed out'],
'💡 Hint: The operation timed out.
- The build or deployment took too long
- Try simplifying your build process', 65, 'timeout');

-- Permission errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['permission denied'],
'💡 Hint: Permission denied error.
- Check file permissions in your repository
- Ensure scripts have executable permissions if needed', 70, 'permission'),

(ARRAY['eacces'],
'💡 Hint: Permission denied (EACCES).
- Check file permissions in your repository
- Ensure scripts have executable permissions if needed', 70, 'permission');

-- Entry point errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['entrypoint'],
'💡 Hint: The entry command could not be executed.
- Check your entry_command setting
- Verify the executable exists in the container
- Ensure the command path is correct', 75, 'entrypoint'),

(ARRAY['command not found'],
'💡 Hint: Command not found.
- Check your entry_command setting
- Verify the executable exists in the container
- Ensure the command path is correct', 75, 'entrypoint'),

(ARRAY['no such file', 'exec'],
'💡 Hint: Executable file not found.
- Check your entry_command setting
- Verify the executable exists in the container', 75, 'entrypoint');

-- Health check errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['health check'],
'💡 Hint: Health check failed.
- Ensure your server starts and listens on the correct port
- For SSE transport: port 3000 (Node), 8000 (Python), 8080 (Go/Rust)
- For STDIO transport: the adapter runs on port 8000
- Check if the server crashes on startup', 85, 'health'),

(ARRAY['healthcheck'],
'💡 Hint: Health check failed.
- Ensure your server starts and listens on the correct port
- For SSE transport: port 3000 (Node), 8000 (Python), 8080 (Go/Rust)
- For STDIO transport: the adapter runs on port 8000', 85, 'health');

-- SSL/TLS errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['ssl'],
'💡 Hint: SSL/TLS error occurred.
- Check if your application makes HTTPS requests with valid certificates
- Verify any external API endpoints are correctly configured', 55, 'ssl'),

(ARRAY['certificate'],
'💡 Hint: Certificate error occurred.
- Check if your application makes HTTPS requests with valid certificates
- Verify any external API endpoints are correctly configured', 55, 'ssl'),

(ARRAY['tls'],
'💡 Hint: TLS error occurred.
- Check if your application makes HTTPS requests with valid certificates
- Verify any external API endpoints are correctly configured', 55, 'ssl');

-- Environment variable errors
INSERT INTO error_hints (keywords, hint_message, priority, category) VALUES
(ARRAY['environment variable'],
'💡 Hint: Required environment variable is missing.
- Add the required secrets in the server settings
- Check your application''s environment requirements', 75, 'env'),

(ARRAY['env var'],
'💡 Hint: Required environment variable is missing.
- Add the required secrets in the server settings', 75, 'env'),

(ARRAY['missing required'],
'💡 Hint: A required configuration is missing.
- Add the required secrets in the server settings
- Check your application''s environment requirements', 60, 'env');
