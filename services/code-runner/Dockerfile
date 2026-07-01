# Sandboxed code runner (NodeFlare code-execution mode).
FROM denoland/deno:alpine

WORKDIR /app
# Build context is the repo root (where fly.code-runner.toml lives).
COPY services/code-runner/main.ts .

# Cache dependencies (none external here) without type-checking; `deno run` transpiles
# and runs TS directly, so we avoid a strict-type build failure.
RUN deno cache main.ts

# The service is trusted: it needs to serve HTTP (--allow-net), read PORT
# (--allow-env=PORT), and spawn the sandbox subprocess (--allow-run=deno). Each user
# execution runs in a child `deno` with far narrower permissions (set in main.ts).
CMD ["run", "--allow-net", "--allow-env=PORT", "--allow-run=deno", "main.ts"]
