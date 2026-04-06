/**
 * Process executor — spawns `claude-code-rs` and reads JSONL from stdout.
 *
 * Modeled after `codex/sdk/typescript/src/exec.ts`.
 */

import { spawn } from "node:child_process";
import path from "node:path";
import readline from "node:readline";
import { execSync } from "node:child_process";

import type { PermissionMode } from "./sessionOptions.js";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ClaudeCodeExecArgs = {
  input: string;
  apiKey?: string;
  model?: string;
  workingDirectory?: string;
  permissionMode?: PermissionMode;
  maxTurns?: number;
  maxBudget?: number;
  systemPrompt?: string;
  appendSystemPrompt?: string;
  verbose?: boolean;
  continueSession?: string;
  signal?: AbortSignal;
};

const INTERNAL_ORIGINATOR_ENV = "CC_RUST_INTERNAL_ORIGINATOR";
const SDK_ORIGINATOR = "claude_code_rs_sdk_ts";

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

export class ClaudeCodeExec {
  private executablePath: string;
  private envOverride?: Record<string, string>;

  constructor(
    executablePath?: string | null,
    env?: Record<string, string>,
  ) {
    this.executablePath = executablePath || findClaudeCodeRsPath();
    this.envOverride = env;
  }

  async *run(args: ClaudeCodeExecArgs): AsyncGenerator<string> {
    const commandArgs: string[] = ["--output-format", "json", "-p"];

    if (args.model) {
      commandArgs.push("--model", args.model);
    }
    if (args.workingDirectory) {
      commandArgs.push("--cwd", args.workingDirectory);
    }
    if (args.permissionMode) {
      commandArgs.push("--permission-mode", args.permissionMode);
    }
    if (args.maxTurns !== undefined) {
      commandArgs.push("--max-turns", String(args.maxTurns));
    }
    if (args.maxBudget !== undefined) {
      commandArgs.push("--max-budget", String(args.maxBudget));
    }
    if (args.systemPrompt) {
      commandArgs.push("--system-prompt", args.systemPrompt);
    }
    if (args.appendSystemPrompt) {
      commandArgs.push("--append-system-prompt", args.appendSystemPrompt);
    }
    if (args.verbose) {
      commandArgs.push("--verbose");
    }
    if (args.continueSession) {
      commandArgs.push("--continue", args.continueSession);
    }

    // Build environment
    const env: Record<string, string> = {};
    if (this.envOverride) {
      Object.assign(env, this.envOverride);
    } else {
      for (const [key, value] of Object.entries(process.env)) {
        if (value !== undefined) {
          env[key] = value;
        }
      }
    }
    if (!env[INTERNAL_ORIGINATOR_ENV]) {
      env[INTERNAL_ORIGINATOR_ENV] = SDK_ORIGINATOR;
    }
    if (args.apiKey) {
      env.ANTHROPIC_API_KEY = args.apiKey;
    }

    const child = spawn(this.executablePath, commandArgs, {
      env,
      signal: args.signal,
    });

    let spawnError: unknown | null = null;
    child.once("error", (err) => (spawnError = err));

    if (!child.stdin) {
      child.kill();
      throw new Error("Child process has no stdin");
    }
    child.stdin.write(args.input);
    child.stdin.end();

    if (!child.stdout) {
      child.kill();
      throw new Error("Child process has no stdout");
    }

    const stderrChunks: Buffer[] = [];
    if (child.stderr) {
      child.stderr.on("data", (data: Buffer) => {
        stderrChunks.push(data);
      });
    }

    const exitPromise = new Promise<{
      code: number | null;
      signal: NodeJS.Signals | null;
    }>((resolve) => {
      child.once("exit", (code, signal) => {
        resolve({ code, signal });
      });
    });

    const rl = readline.createInterface({
      input: child.stdout,
      crlfDelay: Infinity,
    });

    try {
      for await (const line of rl) {
        yield line as string;
      }

      if (spawnError) throw spawnError;
      const { code, signal } = await exitPromise;
      if (code !== 0 || signal) {
        const stderrBuffer = Buffer.concat(stderrChunks);
        const detail = signal ? `signal ${signal}` : `code ${code ?? 1}`;
        throw new Error(
          `claude-code-rs exited with ${detail}: ${stderrBuffer.toString("utf8")}`,
        );
      }
    } finally {
      rl.close();
      child.removeAllListeners();
      try {
        if (!child.killed) child.kill();
      } catch {
        // ignore
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Binary resolution
// ---------------------------------------------------------------------------

function findClaudeCodeRsPath(): string {
  // 1. Explicit env var
  const envPath = process.env["CLAUDE_CODE_RS_PATH"];
  if (envPath) return envPath;

  // 2. Check PATH
  try {
    const cmd =
      process.platform === "win32"
        ? "where claude-code-rs"
        : "which claude-code-rs";
    const result = execSync(cmd, { encoding: "utf8" }).trim();
    if (result) return result.split("\n")[0]!.trim();
  } catch {
    // not on PATH
  }

  // 3. Relative to this package (cargo build output)
  const binaryName =
    process.platform === "win32" ? "claude-code-rs.exe" : "claude-code-rs";
  const candidates = [
    path.resolve(__dirname, "..", "..", "..", "target", "release", binaryName),
    path.resolve(__dirname, "..", "..", "..", "target", "debug", binaryName),
  ];

  for (const candidate of candidates) {
    try {
      const fs = require("node:fs");
      if (fs.existsSync(candidate)) return candidate;
    } catch {
      // continue
    }
  }

  throw new Error(
    "Unable to locate claude-code-rs binary. " +
      "Set CLAUDE_CODE_RS_PATH environment variable or ensure it is on PATH.",
  );
}
