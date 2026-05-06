#!/usr/bin/env node
/**
 * STDIO-to-SSE Adapter for MCP Servers
 *
 * This adapter wraps a STDIO-based MCP server and exposes it as an HTTP/SSE endpoint.
 * It spawns the MCP server as a subprocess and translates between HTTP and STDIO.
 *
 * Usage: node stdio-adapter.cjs <command> [args...]
 *
 * Environment variables:
 * - PORT: HTTP server port (default: 3000)
 * - MCP_PATH: Path prefix for MCP endpoints (default: /mcp)
 */

const http = require('http');
const { spawn } = require('child_process');
const { URL } = require('url');

const PORT = parseInt(process.env.PORT || '3000', 10);
const MCP_PATH = process.env.MCP_PATH || '/mcp';

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
console.log(`[Adapter] Listening on port ${PORT}, MCP path: ${MCP_PATH}`);

// Spawn the MCP server process
let mcpProcess = null;
let messageBuffer = '';
const pendingResponses = new Map(); // id -> { resolve, reject, timeout }
const sseClients = new Set();
let nextRequestId = 1;
let isInitialized = false;
let initializePromise = null;

// Auto-initialize the MCP process
async function autoInitialize() {
  if (isInitialized || !mcpProcess || mcpProcess.killed) {
    return;
  }

  console.log('[Adapter] Auto-initializing MCP process...');

  try {
    // Send initialize request
    const initResponse = await sendToMcpInternal({
      jsonrpc: '2.0',
      method: 'initialize',
      id: nextRequestId++,
      params: {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: {
          name: 'stdio-adapter',
          version: '1.0.0'
        }
      }
    });

    console.log('[Adapter] Initialize response:', JSON.stringify(initResponse));

    // Send initialized notification
    if (mcpProcess && !mcpProcess.killed) {
      mcpProcess.stdin.write(JSON.stringify({
        jsonrpc: '2.0',
        method: 'notifications/initialized'
      }) + '\n');
    }

    isInitialized = true;
    console.log('[Adapter] MCP process initialized successfully');
  } catch (err) {
    console.error('[Adapter] Auto-initialize failed:', err);
    throw err;
  }
}

// Internal send function (doesn't wait for initialization)
function sendToMcpInternal(message) {
  return new Promise((resolve, reject) => {
    if (!mcpProcess || mcpProcess.killed) {
      reject(new Error('MCP process not running'));
      return;
    }

    const id = message.id ?? nextRequestId++;
    const messageWithId = { ...message, id };

    const timeout = setTimeout(() => {
      pendingResponses.delete(id);
      reject(new Error('Request timeout'));
    }, 30000);

    pendingResponses.set(id, { resolve, reject, timeout });

    try {
      mcpProcess.stdin.write(JSON.stringify(messageWithId) + '\n');
    } catch (err) {
      clearTimeout(timeout);
      pendingResponses.delete(id);
      reject(err);
    }
  });
}

function startMcpProcess() {
  console.log(`[Adapter] Spawning MCP process: ${command} ${commandArgs.join(' ')}`);

  isInitialized = false;
  initializePromise = null;

  mcpProcess = spawn(command, commandArgs, {
    stdio: ['pipe', 'pipe', 'inherit'],
    env: { ...process.env },
    shell: false  // Security: Disable shell to prevent command injection
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

    // Clear pending responses
    for (const [id, { reject, timeout }] of pendingResponses) {
      clearTimeout(timeout);
      reject(new Error('MCP process exited'));
    }
    pendingResponses.clear();

    // Restart after a delay
    setTimeout(() => {
      console.log('[Adapter] Restarting MCP process...');
      startMcpProcess();
    }, 1000);
  });

  // Auto-initialize after a short delay to let process start
  setTimeout(() => {
    initializePromise = autoInitialize().catch(err => {
      console.error('[Adapter] Failed to auto-initialize:', err);
    });
  }, 500);
}

function processBuffer() {
  // MCP uses newline-delimited JSON
  const lines = messageBuffer.split('\n');
  messageBuffer = lines.pop() || ''; // Keep incomplete line in buffer

  for (const line of lines) {
    if (!line.trim()) continue;

    try {
      const message = JSON.parse(line);
      handleMcpMessage(message);
    } catch (err) {
      console.error('[Adapter] Failed to parse MCP message:', line, err);
    }
  }
}

function handleMcpMessage(message) {
  // Check if this is a response to a pending request
  if (message.id !== undefined && pendingResponses.has(message.id)) {
    const { resolve, timeout } = pendingResponses.get(message.id);
    clearTimeout(timeout);
    pendingResponses.delete(message.id);
    resolve(message);
    return;
  }

  // Otherwise, it's a notification or server-initiated message
  // Broadcast to all SSE clients
  broadcastToSseClients(message);
}

function broadcastToSseClients(message) {
  const data = JSON.stringify(message);
  console.log(`[Adapter] Broadcasting to ${sseClients.size} SSE clients: ${data.substring(0, 100)}...`);
  for (const client of sseClients) {
    try {
      // MCP SSE transport spec: use "event: message" for JSON-RPC messages
      client.write(`event: message\ndata: ${data}\n\n`);
    } catch (err) {
      console.error('[Adapter] Failed to send to SSE client:', err);
      sseClients.delete(client);
    }
  }
}

// Send message to MCP without waiting for response (for SSE transport)
// Response will be pushed to SSE clients via handleMcpMessage -> broadcastToSseClients
function sendToMcpForSse(message) {
  if (!mcpProcess || mcpProcess.killed) {
    throw new Error('MCP process not running');
  }

  console.log(`[Adapter] Sending to MCP (SSE mode): ${JSON.stringify(message).substring(0, 100)}...`);
  mcpProcess.stdin.write(JSON.stringify(message) + '\n');
}

// Check if message is a notification (no response expected)
// Per JSON-RPC 2.0 spec: A Notification is a Request object without an "id" member
function isNotification(message) {
  return message.id === undefined;
}

// Send notification to MCP (fire-and-forget, no response expected)
async function sendNotificationToMcp(message) {
  // Wait for initialization to complete (if in progress)
  if (initializePromise) {
    await initializePromise;
  }

  if (!mcpProcess || mcpProcess.killed) {
    throw new Error('MCP process not running');
  }

  // If client sends initialized notification and we already sent it, skip
  if (message.method === 'notifications/initialized' && isInitialized) {
    console.log('[Adapter] Skipping duplicate initialized notification');
    return;
  }

  // Send without id, don't wait for response
  mcpProcess.stdin.write(JSON.stringify(message) + '\n');
}

// Send request to MCP and wait for response
// Waits for auto-initialization to complete first
async function sendToMcp(message) {
  // Wait for initialization to complete (if in progress)
  if (initializePromise) {
    await initializePromise;
  }

  // If client sends initialize, and we're already initialized, still forward it
  // (some clients expect to do their own initialization)
  if (message.method === 'initialize' && isInitialized) {
    console.log('[Adapter] Client sent initialize, but already initialized. Forwarding anyway.');
  }

  return sendToMcpInternal(message);
}

// Create HTTP server
const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const path = url.pathname;

  // CORS headers
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  // Health check - only for /health or GET / without SSE Accept header
  if (path === '/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ status: 'ok', transport: 'stdio-adapter' }));
    return;
  }

  // SSE endpoint for receiving messages
  if (path === MCP_PATH || path === `${MCP_PATH}/sse`) {
    if (req.method === 'GET') {
      res.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive'
      });

      sseClients.add(res);
      console.log(`[Adapter] SSE client connected, total: ${sseClients.size}`);

      // Send endpoint event (required by MCP SSE transport spec)
      // This tells the client where to POST messages
      res.write(`event: endpoint\ndata: ${MCP_PATH}/message\n\n`);

      req.on('close', () => {
        sseClients.delete(res);
        console.log(`[Adapter] SSE client disconnected, total: ${sseClients.size}`);
      });

      return;
    }
  }

  // Message endpoint for sending messages to MCP server
  // Supports two modes:
  // 1. SSE transport (if SSE clients connected): POST returns 202, response via SSE stream
  // 2. Synchronous mode (no SSE clients): POST returns 200 with response body
  if (path === `${MCP_PATH}/message` || (path === MCP_PATH && req.method === 'POST')) {
    if (req.method === 'POST') {
      let body = '';
      req.on('data', chunk => { body += chunk; });
      req.on('end', async () => {
        try {
          const message = JSON.parse(body);

          // Wait for initialization to complete (if in progress)
          if (initializePromise) {
            await initializePromise;
          }

          // Check if there are SSE clients connected
          if (sseClients.size > 0) {
            // SSE transport mode: send to MCP, response will come via SSE stream
            sendToMcpForSse(message);
            // Return 202 Accepted with empty body
            res.writeHead(202);
            res.end();
          } else {
            // Synchronous mode (for testing, health checks, etc.)
            // Wait for response and return it directly
            const response = await sendToMcpInternal(message);
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify(response));
          }
        } catch (err) {
          console.error('[Adapter] Error processing message:', err);
          // JSON-RPC 2.0 error codes:
          // -32700: Parse error (invalid JSON)
          // -32600: Invalid Request
          // -32603: Internal error
          // -32000 to -32099: Server error (implementation-defined)
          let errorCode = -32603; // Default: Internal error
          if (err instanceof SyntaxError) {
            errorCode = -32700; // Parse error
          } else if (err.message?.includes('not running')) {
            errorCode = -32000; // Server error: MCP process not running
          }
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: errorCode, message: err.message },
            id: null
          }));
        }
      });
      return;
    }
  }

  // Fallback health check for GET / without SSE Accept header (for Fly.io health checks)
  if (path === '/' && req.method === 'GET') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ status: 'ok', transport: 'stdio-adapter' }));
    return;
  }

  // Not found
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
