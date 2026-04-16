/**
 * Simple non-streaming example.
 *
 * Usage:
 *   npx ts-node --esm samples/simple_run.ts
 */

import { ClaudeCode } from "../src/index.js";

async function main() {
  const client = new ClaudeCode();
  const session = client.startSession({
    permissionMode: "auto",
  });

  const turn = await session.run("What files are in the current directory?");

  console.log("Session ID:", session.sessionId);
  console.log("Response:", turn.finalResponse);
  console.log("Items:", turn.items.length);
  if (turn.usage) {
    console.log("Tokens:", turn.usage.input_tokens, "in /", turn.usage.output_tokens, "out");
    console.log("Cost: $", turn.usage.total_cost_usd.toFixed(4));
  }
}

main().catch(console.error);
