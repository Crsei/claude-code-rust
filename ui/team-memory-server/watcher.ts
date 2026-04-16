// ui/team-memory-server/watcher.ts
import { watch, existsSync, mkdirSync } from "fs";
import type { SyncState } from "./sync";
import { push } from "./sync";

const DEBOUNCE_MS = 2000;

let debounceTimer: Timer | null = null;

function resetDebounceTimer(fn: () => void): void {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(fn, DEBOUNCE_MS);
}

export function startWatcher(state: SyncState): void {
  if (!existsSync(state.teamMemPath)) {
    mkdirSync(state.teamMemPath, { recursive: true });
  }

  try {
    const watcher = watch(state.teamMemPath, { recursive: true }, (_eventType, filename) => {
      if (!filename) return;

      const key = filename.replace(/\\/g, "/");

      if (state.suppressSet.has(key)) {
        state.suppressSet.delete(key);
        return;
      }

      resetDebounceTimer(() => {
        try {
          push(state);
        } catch (err) {
          console.error("team-memory-sync: push error:", err);
        }
      });
    });

    process.on("SIGTERM", () => watcher.close());
    process.on("SIGINT", () => watcher.close());

    console.log(`team-memory-sync: watching ${state.teamMemPath}`);
  } catch (err) {
    console.error("team-memory-sync: failed to start watcher:", err);
  }
}

export function flushPendingPush(state: SyncState): void {
  if (debounceTimer) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
    try {
      push(state);
    } catch (err) {
      console.error("team-memory-sync: flush error:", err);
    }
  }
}
