/**
 * Interactive streaming example.
 *
 * Usage:
 *   npx ts-node --esm samples/basic_streaming.ts
 */

import readline from "node:readline";
import { ClaudeCode } from "../src/index.js";

async function main() {
  const client = new ClaudeCode();
  const session = client.startSession({
    permissionMode: "auto",
  });

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const prompt = (query: string): Promise<string> =>
    new Promise((resolve) => rl.question(query, resolve));

  console.log("Claude Code RS SDK — Interactive Demo");
  console.log("Type your message (Ctrl+C to exit)\n");

  while (true) {
    const input = await prompt("> ");
    if (!input.trim()) continue;

    const { events } = await session.runStreamed(input);

    for await (const event of events) {
      switch (event.type) {
        case "session.started":
          console.log(`[Session ${event.session_id} | Model: ${event.model}]`);
          break;
        case "stream.delta":
          if (event.event_type === "content_block_delta" && event.delta) {
            const delta = event.delta as { text?: string };
            if (delta.text) {
              process.stdout.write(delta.text);
            }
          }
          break;
        case "item.completed":
          if (event.item.type === "tool_use_summary") {
            console.log(`\n[Tool] ${event.item.summary}`);
          }
          break;
        case "turn.completed":
          console.log(
            `\n[Done: ${event.num_turns} turn(s), $${event.usage.total_cost_usd.toFixed(4)}]\n`,
          );
          break;
        case "turn.failed":
          console.error(`\n[Error: ${event.error.message}]\n`);
          break;
        case "error":
          if (event.retryable) {
            console.log(
              `\n[Retry ${event.attempt}/${event.max_retries}: ${event.message}]`,
            );
          }
          break;
      }
    }
  }
}

main().catch(console.error);
