#!/usr/bin/env node
/**
 * STDIO-to-HTTP Adapter for MCP Servers
 *
 * Wraps a STDIO-based MCP server and exposes it over HTTP using the modern
 * **Streamable HTTP** transport (single endpoint, request/response in the POST
 * body, `Mcp-Session-Id` header, optional GET stream for server->client messages).
 *
 * The deprecated 2024-11-05 HTTP+SSE transport (GET stream that emits
 * `event: endpoint` + separate `POST /mcp/message`) is kept working for older
 * clients (dual support).
 *
 * Usage: node stdio-adapter.cjs <command> [args...]
 *
 * Environment variables:
 * - PORT: HTTP server port (default: 3000)
 * - MCP_PATH: Path of the MCP endpoint (default: /mcp)
 */

const http = require('http');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { URL } = require('url');

const PORT = parseInt(process.env.PORT || '3000', 10);
const MCP_PATH = process.env.MCP_PATH || '/mcp';
const REQUEST_TIMEOUT_MS = 30000;
const KEEPALIVE_MS = 25000;

// Get the command to run from arguments
const args = process.argv.slice(2);
if (args.length === 0) {
  console.error('Usage: node stdio-adapter.cjs <command> [args...]');
  console.error('Example: node stdio-adapter.cjs python main.py');
  process.exit(1);
}

const command = args[0];
const commandArgs = args.slice(1);

// Security: Validate command name contains only safe characters
// This prevents shell injection even though shell: false is used
const SAFE_COMMAND_REGEX = /^[a-zA-Z0-9_\-./]+$/;
if (!SAFE_COMMAND_REGEX.test(command)) {
  console.error(`[Adapter] Invalid command name: ${command}`);
  process.exit(1);
}
for (const arg of commandArgs) {
  // Allow typical argument characters but block obvious shell metacharacters
  if (/[;&|`$(){}]/.test(arg)) {
    console.error(`[Adapter] Invalid characters in argument: ${arg}`);
    process.exit(1);
  }
}

console.log(`[Adapter] Starting STDIO adapter for: ${command} ${commandArgs.join(' ')}`);
console.log(`[Adapter] Listening on port ${PORT}, MCP path: ${MCP_PATH} (Streamable HTTP + legacy SSE)`);

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
let mcpProcess = null;
let messageBuffer = '';
let isInitialized = false;
let initializePromise = null;

// Single child process is shared by all callers. To avoid cross-request id
// collisions we remap every *client request* id to a unique internal id and
// restore the original id on the matching response.
//   pending: internalId -> { resolve, reject, timeout, clientId }
const pending = new Map();
let nextInternalId = 1;

// Open server->client streams (GET on MCP_PATH / legacy /sse). Used to deliver
// server-initiated messages and legacy SSE responses.
const streams = new Set();

// Streamable HTTP session id (single shared child => single logical session).
let sessionId = null;

// ---------------------------------------------------------------------------
// Talking to the child process
// ---------------------------------------------------------------------------

// Send a request to the child and resolve with its response (id-remapped).
function sendRequest(message) {
  return new Promise((resolve, reject) => {
    if (!mcpProcess || mcpProcess.killed) {
      reject(new Error('MCP process not running'));
      return;
    }
    const internalId = `nf-${nextInternalId++}`;
    const clientId = message.id;
    const timeout = setTimeout(() => {
      pending.delete(internalId);
      reject(new Error('Request timeout'));
    }, REQUEST_TIMEOUT_MS);
    pending.set(internalId, { resolve, reject, timeout, clientId });

    const outgoing = { ...message, id: internalId };
    try {
      mcpProcess.stdin.write(JSON.stringify(outgoing) + '\n');
    } catch (err) {
      clearTimeout(timeout);
      pending.delete(internalId);
      reject(err);
    }
  });
}

// Fire-and-forget write to the child (notifications, or client responses to
// server-initiated requests, or legacy-mode requests). No id remapping: the
// child's reply (if any) flows back out via broadcastToStreams.
function sendRaw(message) {
  if (!mcpProcess || mcpProcess.killed) {
    throw new Error('MCP process not running');
  }
  mcpProcess.stdin.write(JSON.stringify(message) + '\n');
}

function broadcastToStreams(message) {
  if (streams.size === 0) return;
  const data = JSON.stringify(message);
  for (const client of streams) {
    try {
      client.write(`event: message\ndata: ${data}\n\n`);
    } catch (err) {
      console.error('[Adapter] Failed to send to stream client:', err);
      streams.delete(client);
    }
  }
}

// Auto-initialize the child once on startup so it's warm and we can detect a
// healthy process. Clients still send their own initialize, which is forwarded.
async function autoInitialize() {
  if (isInitialized || !mcpProcess || mcpProcess.killed) {
    return;
  }
  console.log('[Adapter] Auto-initializing MCP process...');
  try {
    await sendRequest({
      jsonrpc: '2.0',
      method: 'initialize',
      params: {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'stdio-adapter', version: '1.0.0' },
      },
    });
    if (mcpProcess && !mcpProcess.killed) {
      sendRaw({ jsonrpc: '2.0', method: 'notifications/initialized' });
    }
    isInitialized = true;
    console.log('[Adapter] MCP process initialized successfully');
  } catch (err) {
    console.error('[Adapter] Auto-initialize failed:', err);
    throw err;
  }
}

function processBuffer() {
  // MCP uses newline-delimited JSON
  const lines = messageBuffer.split('\n');
  messageBuffer = lines.pop() || ''; // Keep incomplete line in buffer
  for (const line of lines) {
    if (!line.trim()) continue;
    try {
      handleMcpMessage(JSON.parse(line));
    } catch (err) {
      console.error('[Adapter] Failed to parse MCP message:', line, err);
    }
  }
}

function handleMcpMessage(message) {
  // Response to one of our remapped requests?
  if (message.id !== undefined && message.id !== null && pending.has(message.id)) {
    const { resolve, timeout, clientId } = pending.get(message.id);
    clearTimeout(timeout);
    pending.delete(message.id);
    resolve({ ...message, id: clientId }); // restore the caller's original id
    return;
  }
  // Otherwise it's a server-initiated request/notification, or a legacy-mode
  // response: push it to any open server->client stream.
  broadcastToStreams(message);
}

function startMcpProcess() {
  console.log(`[Adapter] Spawning MCP process: ${command} ${commandArgs.join(' ')}`);
  isInitialized = false;
  initializePromise = null;

  mcpProcess = spawn(command, commandArgs, {
    stdio: ['pipe', 'pipe', 'inherit'],
    env: { ...process.env },
    shell: false, // Security: Disable shell to prevent command injection
  });

  mcpProcess.stdout.on('data', (data) => {
    messageBuffer += data.toString();
    processBuffer();
  });

  mcpProcess.on('error', (err) => {
    console.error('[Adapter] MCP process error:', err);
  });

  mcpProcess.on('exit', (code, signal) => {
    console.log(`[Adapter] MCP process exited with code ${code}, signal ${signal}`);
    isInitialized = false;
    initializePromise = null;
    sessionId = null;

    // Reject pending requests
    for (const [, { reject, timeout }] of pending) {
      clearTimeout(timeout);
      reject(new Error('MCP process exited'));
    }
    pending.clear();

    // Restart after a delay
    setTimeout(() => {
      console.log('[Adapter] Restarting MCP process...');
      startMcpProcess();
    }, 1000);
  });

  // Auto-initialize after a short delay to let the process start
  setTimeout(() => {
    initializePromise = autoInitialize().catch((err) => {
      console.error('[Adapter] Failed to auto-initialize:', err);
    });
  }, 500);
}

// ---------------------------------------------------------------------------
// HTTP server
// ---------------------------------------------------------------------------

function isRequestMessage(m) {
  return m && typeof m.method === 'string' && m.id !== undefined && m.id !== null;
}

function openStream(req, res) {
  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
    'X-Accel-Buffering': 'no',
  });
  streams.add(res);
  console.log(`[Adapter] Stream opened, total: ${streams.size}`);

  // Legacy SSE transport: tell old clients where to POST. Streamable HTTP
  // clients ignore unknown SSE events, so this is harmless to them.
  res.write(`event: endpoint\ndata: ${MCP_PATH}/message\n\n`);

  const keepalive = setInterval(() => {
    try {
      res.write(`:ping\n\n`);
    } catch (err) {
      clearInterval(keepalive);
    }
  }, KEEPALIVE_MS);

  req.on('close', () => {
    clearInterval(keepalive);
    streams.delete(res);
    console.log(`[Adapter] Stream closed, total: ${streams.size}`);
  });
}

async function handlePost(req, res, path) {
  let body = '';
  req.on('data', (chunk) => {
    body += chunk;
  });
  req.on('end', async () => {
    try {
      const parsed = JSON.parse(body);

      // Make sure the child is up before forwarding.
      if (initializePromise) {
        await initializePromise.catch(() => {});
      }

      const clientSession = req.headers['mcp-session-id'];
      // Legacy detection (behind the proxy the path is flattened to MCP_PATH, so
      // we cannot rely on /message): treat as legacy when the request explicitly
      // used the legacy message path, OR it carries no session id while a
      // server->client stream is already open (the legacy client opens the SSE
      // stream first and never sends Mcp-Session-Id).
      const legacy =
        path === `${MCP_PATH}/message` || (!clientSession && streams.size > 0);

      const messages = Array.isArray(parsed) ? parsed : [parsed];
      const requests = messages.filter(isRequestMessage);
      const others = messages.filter((m) => !isRequestMessage(m));

      // Forward notifications / client responses (no reply expected here).
      for (const m of others) {
        if (m && m.method === 'notifications/initialized' && isInitialized) {
          continue; // child was already initialized by autoInitialize
        }
        try {
          sendRaw(m);
        } catch (err) {
          console.error('[Adapter] Failed to forward message:', err);
        }
      }

      // No requests -> nothing to wait for.
      if (requests.length === 0) {
        res.writeHead(202);
        res.end();
        return;
      }

      if (legacy) {
        // Legacy SSE: forward with original ids; responses are delivered on the
        // client's open SSE stream via broadcastToStreams.
        for (const m of requests) sendRaw(m);
        res.writeHead(202);
        res.end();
        return;
      }

      // Streamable HTTP: await responses and return them in the POST body.
      const responses = await Promise.all(requests.map((m) => sendRequest(m)));

      const headers = { 'Content-Type': 'application/json' };
      if (requests.some((m) => m.method === 'initialize') && !sessionId) {
        sessionId = crypto.randomUUID();
      }
      if (sessionId) headers['Mcp-Session-Id'] = sessionId;

      const payload = Array.isArray(parsed) ? responses : responses[0];
      res.writeHead(200, headers);
      res.end(JSON.stringify(payload));
    } catch (err) {
      console.error('[Adapter] Error processing message:', err);
      // JSON-RPC 2.0 error codes: -32700 parse, -32000 server, -32603 internal
      let errorCode = -32603;
      let statusCode = 500;
      if (err instanceof SyntaxError) {
        errorCode = -32700;
        statusCode = 400;
      } else if (err.message?.includes('not running')) {
        errorCode = -32000;
      }
      res.writeHead(statusCode, { 'Content-Type': 'application/json' });
      res.end(
        JSON.stringify({
          jsonrpc: '2.0',
          error: { code: errorCode, message: err.message },
          id: null,
        })
      );
    }
  });
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const path = url.pathname;

  // CORS
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS');
  res.setHeader(
    'Access-Control-Allow-Headers',
    'Content-Type, Authorization, Mcp-Session-Id, MCP-Protocol-Version, Last-Event-ID'
  );
  res.setHeader('Access-Control-Expose-Headers', 'Mcp-Session-Id');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  // Health check
  if (path === '/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ status: 'ok', transport: 'stdio-adapter' }));
    return;
  }

  // GET stream: Streamable HTTP server->client stream (also serves legacy /sse).
  if ((path === MCP_PATH || path === `${MCP_PATH}/sse`) && req.method === 'GET') {
    openStream(req, res);
    return;
  }

  // DELETE: Streamable HTTP session termination.
  if (path === MCP_PATH && req.method === 'DELETE') {
    sessionId = null;
    res.writeHead(204);
    res.end();
    return;
  }

  // POST: messages (Streamable HTTP at MCP_PATH; legacy at MCP_PATH/message).
  if ((path === MCP_PATH || path === `${MCP_PATH}/message`) && req.method === 'POST') {
    await handlePost(req, res, path);
    return;
  }

  // Fallback health check for GET / (Fly.io health checks)
  if (path === '/' && req.method === 'GET') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ status: 'ok', transport: 'stdio-adapter' }));
    return;
  }

  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'Not found' }));
});

// Start the server and MCP process
server.listen(PORT, '0.0.0.0', () => {
  console.log(`[Adapter] HTTP server listening on 0.0.0.0:${PORT}`);
  startMcpProcess();
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('[Adapter] Received SIGTERM, shutting down...');
  if (mcpProcess) {
    mcpProcess.kill('SIGTERM');
  }
  server.close(() => {
    process.exit(0);
  });
});

process.on('SIGINT', () => {
  console.log('[Adapter] Received SIGINT, shutting down...');
  if (mcpProcess) {
    mcpProcess.kill('SIGINT');
  }
  server.close(() => {
    process.exit(0);
  });
});
