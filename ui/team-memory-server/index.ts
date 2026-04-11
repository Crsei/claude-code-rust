// ui/team-memory-server/index.ts
import * as db from "./db";
import { handleGet, handlePut } from "./routes";
import { mkdirSync } from "fs";
import { join } from "path";
import { homedir } from "os";

// --- CLI args parsing ---
function parseArgs(): { port: number; secret: string } {
  const args = process.argv.slice(2);
  let port = 19837;
  let secret = "";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--port" && args[i + 1]) {
      port = parseInt(args[i + 1], 10);
      i++;
    } else if (args[i] === "--secret" && args[i + 1]) {
      secret = args[i + 1];
      i++;
    }
  }
  if (!secret) {
    console.error("error: --secret is required");
    process.exit(1);
  }
  return { port, secret };
}

const { port, secret } = parseArgs();

// --- Database init ---
const dataDir = join(homedir(), ".cc-rust");
mkdirSync(dataDir, { recursive: true });

const dbPath = join(dataDir, "team-memory.db");
db.init(dbPath);

// --- Graceful shutdown ---
process.on("SIGTERM", () => {
  console.log("team-memory-server: shutting down");
  db.close();
  process.exit(0);
});
process.on("SIGINT", () => {
  db.close();
  process.exit(0);
});

// --- HTTP Server ---
const server = Bun.serve({
  port,
  hostname: "127.0.0.1",
  async fetch(req) {
    const url = new URL(req.url);

    // Health endpoint (no auth)
    if (url.pathname === "/health") {
      return Response.json({ status: "ok" });
    }

    // Shared secret auth
    if (req.headers.get("x-team-memory-secret") !== secret) {
      return Response.json({ error: "unauthorized" }, { status: 401 });
    }

    // Route dispatch
    if (url.pathname === "/api/claude_code/team_memory") {
      if (req.method === "GET") return handleGet(url, req);
      if (req.method === "PUT") return handlePut(url, req);
      return Response.json({ error: "method not allowed" }, { status: 405 });
    }

    return Response.json({ error: "not found" }, { status: 404 });
  },
});

console.log(`team-memory-server listening on http://127.0.0.1:${server.port}`);
