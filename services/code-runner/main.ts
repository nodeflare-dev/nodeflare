// Sandboxed code runner for NodeFlare code-execution mode.
//
// Runs as its own Fly app (= Firecracker microVM), separate from the proxy. It accepts
// AI-written JavaScript and executes each request in a FRESH, locked-down Deno
// subprocess:
//   deno run --allow-net=<tools-endpoint-host> --no-prompt -
// i.e. no file system, no env, no subprocess, and network restricted to ONLY the
// proxy's scope-enforced tool-call endpoint. The injected `tools.*` API POSTs there
// with the per-execution token; that endpoint — not this wrapper — is the security
// boundary (it re-checks scope server-side). Even if user code bypasses `tools` and
// calls fetch() directly, --allow-net limits it to that one host and the token only
// grants the execution's own scope.
//
// This service itself is trusted code; it runs with --allow-net (to serve) and
// --allow-run=deno (to spawn the sandbox). It holds NO secrets.

const SERVICE_PORT = Number(Deno.env.get("PORT") ?? "8080");

interface RunRequest {
  code: string;
  token: string;
  tools_endpoint: string;
  timeout_secs?: number;
  max_tool_calls?: number;
}

function bootstrap(req: RunRequest, guid: string): string {
  const maxCalls = req.max_tool_calls ?? 50;
  // Values are JSON-encoded so embedding them as source is safe. The user code is
  // appended as-is and executed inside an async IIFE; the sandbox (not escaping) is
  // the security boundary, so we don't need to sanitize it.
  return `
const __TOKEN = ${JSON.stringify(req.token)};
const __ENDPOINT = ${JSON.stringify(req.tools_endpoint)};
const __MAX = ${maxCalls};
const __GUID = ${JSON.stringify(guid)};
// Route user console output to stderr so stdout carries only our result frame.
console.log = (...a) => console.error(...a);
console.info = (...a) => console.error(...a);
console.debug = (...a) => console.error(...a);
let __calls = 0;
const tools = new Proxy({}, {
  get(_t, name) {
    return async (args) => {
      if (typeof name !== "string") throw new Error("invalid tool name");
      if (++__calls > __MAX) throw new Error("tool call limit (" + __MAX + ") exceeded");
      const res = await fetch(__ENDPOINT, {
        method: "POST",
        headers: { "content-type": "application/json", "authorization": "Bearer " + __TOKEN },
        body: JSON.stringify({ tool: name, arguments: args ?? {} }),
      });
      if (!res.ok) throw new Error("tool '" + name + "' failed: HTTP " + res.status);
      return await res.json();
    };
  },
});
const __emit = (marker, payload) =>
  Deno.stdout.write(new TextEncoder().encode("\\n" + __GUID + marker + "\\n" + JSON.stringify(payload ?? null)));
try {
  const __result = await (async () => { ${req.code}
  })();
  await __emit(":OK:", __result ?? null);
} catch (e) {
  await __emit(":ERR:", String((e && e.message) || e));
}
`;
}

async function runSandboxed(req: RunRequest): Promise<{ output?: unknown; error?: string }> {
  const t0 = Date.now();
  let host: string;
  try {
    host = new URL(req.tools_endpoint).host;
  } catch {
    console.error(`[run] invalid tools_endpoint: ${req.tools_endpoint}`);
    return { error: "invalid tools_endpoint" };
  }
  const guid = crypto.randomUUID();
  const program = bootstrap(req, guid);
  const timeoutMs = (req.timeout_secs ?? 15) * 1000;
  console.error(
    `[run] start: code=${req.code.length}B allow-net=${host} timeout=${timeoutMs}ms max_calls=${req.max_tool_calls ?? 50}`,
  );

  const command = new Deno.Command("deno", {
    args: ["run", "--no-prompt", `--allow-net=${host}`, "-"],
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
  });
  const child = command.spawn();

  // Feed the program via stdin (no temp files, no fs permission needed).
  const writer = child.stdin.getWriter();
  await writer.write(new TextEncoder().encode(program));
  await writer.close();

  // Enforce a wall-clock timeout: kill the subprocess tree if it overruns.
  const timer = setTimeout(() => {
    try {
      child.kill("SIGKILL");
    } catch { /* already exited */ }
  }, timeoutMs);

  const { code: exitCode, stdout, stderr } = await child.output();
  clearTimeout(timer);
  const ms = Date.now() - t0;

  // Sandbox logs (user console.* + tool-call errors) land on stderr.
  const errText = new TextDecoder().decode(stderr).trim();
  if (errText) {
    console.error(`[run] sandbox stderr:\n${errText}`);
  }

  const text = new TextDecoder().decode(stdout);
  const okMarker = "\n" + guid + ":OK:\n";
  const errMarker = "\n" + guid + ":ERR:\n";

  const okIdx = text.lastIndexOf(okMarker);
  if (okIdx !== -1) {
    const json = text.slice(okIdx + okMarker.length);
    console.error(`[run] done OK in ${ms}ms (result=${json.length}B)`);
    try {
      return { output: JSON.parse(json) };
    } catch {
      return { output: json };
    }
  }
  const errIdx = text.lastIndexOf(errMarker);
  if (errIdx !== -1) {
    const json = text.slice(errIdx + errMarker.length);
    console.error(`[run] done ERR in ${ms}ms: ${json}`);
    try {
      return { error: String(JSON.parse(json)) };
    } catch {
      return { error: json };
    }
  }
  console.error(`[run] no result in ${ms}ms (exit=${exitCode}, timed out or crashed)`);
  return { error: "execution produced no result (timed out or crashed)" };
}

// Bind IPv6 (dual-stack): Fly private networking (`.internal`) is IPv6-only, so the
// proxy reaches us over 6PN. `0.0.0.0` (IPv4) would refuse those connections.
Deno.serve({ port: SERVICE_PORT, hostname: "::" }, async (req) => {
  const url = new URL(req.url);
  if (req.method === "GET" && url.pathname === "/health") {
    return new Response("ok");
  }
  if (req.method === "POST" && url.pathname === "/run") {
    console.error("[run] request received");
    let body: RunRequest;
    try {
      body = await req.json();
    } catch {
      console.error("[run] rejected: invalid JSON body");
      return Response.json({ error: "invalid JSON body" }, { status: 400 });
    }
    if (!body.code || !body.token || !body.tools_endpoint) {
      console.error("[run] rejected: missing code/token/tools_endpoint");
      return Response.json({ error: "missing code/token/tools_endpoint" }, { status: 400 });
    }
    const result = await runSandboxed(body);
    return Response.json(result);
  }
  return new Response("not found", { status: 404 });
});
