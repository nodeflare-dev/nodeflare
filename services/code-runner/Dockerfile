# Sandboxed code runner (NodeFlare code-execution mode).
FROM denoland/deno:alpine-2.1.4

WORKDIR /app
COPY main.ts .

# Cache/type-check the entrypoint at build time.
RUN deno check main.ts

# The service is trusted: it needs to serve HTTP (--allow-net), read PORT
# (--allow-env=PORT), and spawn the sandbox subprocess (--allow-run=deno). Each user
# execution runs in a child `deno` with far narrower permissions (set in main.ts).
CMD ["run", "--allow-net", "--allow-env=PORT", "--allow-run=deno", "main.ts"]
