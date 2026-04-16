// ui/team-memory-server/db.ts
import { Database } from "bun:sqlite";
import { createHash } from "crypto";

let db: Database;

export function init(dbPath: string): void {
  db = new Database(dbPath, { create: true });
  db.exec("PRAGMA journal_mode=WAL");
  db.exec("PRAGMA foreign_keys=ON");
  db.exec(`
    CREATE TABLE IF NOT EXISTS team_memory (
      repo       TEXT NOT NULL,
      key        TEXT NOT NULL,
      content    TEXT NOT NULL,
      checksum   TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY (repo, key)
    )
  `);
  db.exec(`
    CREATE TABLE IF NOT EXISTS repo_meta (
      repo       TEXT PRIMARY KEY,
      version    INTEGER NOT NULL DEFAULT 1,
      checksum   TEXT NOT NULL,
      updated_at TEXT NOT NULL
    )
  `);
}

export function close(): void {
  db?.close();
}

export function getRepoMeta(repo: string): { version: number; checksum: string; updated_at: string } | null {
  return db.query("SELECT version, checksum, updated_at FROM repo_meta WHERE repo = ?").get(repo) as any;
}

export function getEntries(repo: string): Record<string, string> {
  const rows = db.query("SELECT key, content FROM team_memory WHERE repo = ?").all(repo) as { key: string; content: string }[];
  const result: Record<string, string> = {};
  for (const row of rows) {
    result[row.key] = row.content;
  }
  return result;
}

export function getEntryChecksums(repo: string): Record<string, string> {
  const rows = db.query("SELECT key, checksum FROM team_memory WHERE repo = ?").all(repo) as { key: string; checksum: string }[];
  const result: Record<string, string> = {};
  for (const row of rows) {
    result[row.key] = row.checksum;
  }
  return result;
}

export function getEntryCount(repo: string): number {
  const row = db.query("SELECT COUNT(*) as cnt FROM team_memory WHERE repo = ?").get(repo) as { cnt: number };
  return row.cnt;
}

export function hasEntry(repo: string, key: string): boolean {
  const row = db.query("SELECT 1 FROM team_memory WHERE repo = ? AND key = ?").get(repo, key);
  return row !== null;
}

export function sha256(content: string): string {
  return `sha256:${createHash("sha256").update(content, "utf-8").digest("hex")}`;
}

export function computeGlobalChecksum(repo: string): string {
  const checksums = getEntryChecksums(repo);
  const sorted = Object.entries(checksums)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, hash]) => `${key}:${hash}`)
    .join("\n");
  return sha256(sorted);
}

export function upsertEntries(repo: string, entries: Record<string, string>): string {
  const now = new Date().toISOString();
  const upsert = db.prepare(
    "INSERT INTO team_memory (repo, key, content, checksum, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(repo, key) DO UPDATE SET content=excluded.content, checksum=excluded.checksum, updated_at=excluded.updated_at"
  );
  const upsertMeta = db.prepare(
    "INSERT INTO repo_meta (repo, version, checksum, updated_at) VALUES (?, 1, ?, ?) ON CONFLICT(repo) DO UPDATE SET version=version+1, checksum=excluded.checksum, updated_at=excluded.updated_at"
  );

  const tx = db.transaction(() => {
    for (const [key, content] of Object.entries(entries)) {
      const checksum = sha256(content);
      upsert.run(repo, key, content, checksum, now);
    }
    const globalChecksum = computeGlobalChecksum(repo);
    upsertMeta.run(repo, globalChecksum, now);
    return globalChecksum;
  });

  return tx();
}
