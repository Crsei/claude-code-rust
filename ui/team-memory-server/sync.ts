// ui/team-memory-server/sync.ts
import { readFileSync, writeFileSync, readdirSync, existsSync, mkdirSync, statSync } from "fs";
import { join, relative, dirname } from "path";
import * as db from "./db";

const MAX_FILE_SIZE = 250_000;

export interface SyncState {
  lastKnownChecksum: string | null;
  serverChecksums: Map<string, string>;
  teamMemPath: string;
  repo: string;
  suppressSet: Set<string>;
}

export function createSyncState(repo: string, teamMemPath: string): SyncState {
  return {
    lastKnownChecksum: null,
    serverChecksums: new Map(),
    teamMemPath,
    repo,
    suppressSet: new Set(),
  };
}

export function pull(state: SyncState): void {
  const entries = db.getEntries(state.repo);
  const checksums = db.getEntryChecksums(state.repo);
  const meta = db.getRepoMeta(state.repo);

  state.serverChecksums.clear();
  for (const [key, hash] of Object.entries(checksums)) {
    state.serverChecksums.set(key, hash);
  }
  state.lastKnownChecksum = meta?.checksum ?? null;

  writeEntriesToLocal(entries, state);

  const count = Object.keys(entries).length;
  if (count > 0) {
    console.log(`team-memory-sync: pulled ${count} entries to ${state.teamMemPath}`);
  }
}

function writeEntriesToLocal(entries: Record<string, string>, state: SyncState): void {
  for (const [key, content] of Object.entries(entries)) {
    if (new TextEncoder().encode(content).byteLength > MAX_FILE_SIZE) {
      console.warn(`team-memory-sync: skipping oversized entry: ${key}`);
      continue;
    }

    const filePath = join(state.teamMemPath, key);

    if (existsSync(filePath)) {
      try {
        const local = readFileSync(filePath, "utf-8");
        if (local === content) continue;
      } catch {
        // File unreadable, overwrite it
      }
    }

    state.suppressSet.add(key);

    const dir = dirname(filePath);
    mkdirSync(dir, { recursive: true });

    writeFileSync(filePath, content, "utf-8");
  }
}

export function push(state: SyncState): void {
  const localEntries = readLocalTeamMemory(state.teamMemPath);

  const delta: Record<string, string> = {};
  for (const [key, { content, checksum }] of localEntries) {
    const serverHash = state.serverChecksums.get(key);
    if (serverHash !== checksum) {
      delta[key] = content;
    }
  }

  if (Object.keys(delta).length === 0) return;

  const newChecksum = db.upsertEntries(state.repo, delta);

  state.lastKnownChecksum = newChecksum;
  const newChecksums = db.getEntryChecksums(state.repo);
  state.serverChecksums.clear();
  for (const [key, hash] of Object.entries(newChecksums)) {
    state.serverChecksums.set(key, hash);
  }

  console.log(`team-memory-sync: pushed ${Object.keys(delta).length} entries`);
}

function readLocalTeamMemory(
  teamMemPath: string
): Map<string, { content: string; checksum: string }> {
  const result = new Map<string, { content: string; checksum: string }>();

  if (!existsSync(teamMemPath)) return result;

  function walk(dir: string): void {
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      return;
    }
    for (const entry of entries) {
      const fullPath = join(dir, entry);
      let stat;
      try {
        stat = statSync(fullPath);
      } catch {
        continue;
      }
      if (stat.isDirectory()) {
        walk(fullPath);
      } else if (stat.isFile()) {
        if (stat.size > MAX_FILE_SIZE) continue;
        try {
          const content = readFileSync(fullPath, "utf-8");
          const key = relative(teamMemPath, fullPath).replace(/\\/g, "/");
          const checksum = db.sha256(content);
          result.set(key, { content, checksum });
        } catch {
          // Skip unreadable files
        }
      }
    }
  }

  walk(teamMemPath);
  return result;
}
