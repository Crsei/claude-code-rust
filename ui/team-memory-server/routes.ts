// ui/team-memory-server/routes.ts
import * as db from "./db";

const MAX_ENTRIES = 500;
const MAX_ENTRY_SIZE = 250_000;
const MAX_PUT_BODY = 512_000;

export async function handleGet(url: URL, req: Request): Promise<Response> {
  const repo = url.searchParams.get("repo");
  if (!repo) {
    return Response.json({ error: "missing repo parameter" }, { status: 400 });
  }

  const view = url.searchParams.get("view");
  const meta = db.getRepoMeta(repo);
  const currentChecksum = meta?.checksum ?? "";

  // Conditional request: If-None-Match
  const ifNoneMatch = req.headers.get("if-none-match");
  if (meta && ifNoneMatch && ifNoneMatch === currentChecksum) {
    return new Response(null, { status: 304, headers: { ETag: currentChecksum } });
  }

  if (view === "hashes") {
    const entryChecksums = db.getEntryChecksums(repo);
    return Response.json(
      { repo, checksum: currentChecksum, entryChecksums },
      { headers: { ETag: currentChecksum } }
    );
  }

  const entries = db.getEntries(repo);
  const entryChecksums = db.getEntryChecksums(repo);
  return Response.json(
    {
      repo,
      version: meta?.version ?? 0,
      lastModified: meta?.updated_at ?? "",
      checksum: currentChecksum,
      content: { entries, entryChecksums },
    },
    { headers: { ETag: currentChecksum } }
  );
}

export async function handlePut(url: URL, req: Request): Promise<Response> {
  const repo = url.searchParams.get("repo");
  if (!repo) {
    return Response.json({ error: "missing repo parameter" }, { status: 400 });
  }

  // Body size check
  const contentLength = parseInt(req.headers.get("content-length") ?? "0", 10);
  if (contentLength > MAX_PUT_BODY) {
    return Response.json({ error: "body too large" }, { status: 413 });
  }

  // ETag check
  const ifMatch = req.headers.get("if-match");
  const meta = db.getRepoMeta(repo);
  if (meta && ifMatch !== meta.checksum && ifMatch !== "*") {
    return Response.json(
      { error: "checksum_mismatch", current_checksum: meta.checksum },
      { status: 412 }
    );
  }
  // First write: allow if no meta exists (ifMatch can be "" or "*" or absent)
  if (!meta && ifMatch && ifMatch !== "" && ifMatch !== "*") {
    return Response.json(
      { error: "checksum_mismatch", current_checksum: "" },
      { status: 412 }
    );
  }

  const body = (await req.json()) as { entries: Record<string, string> };
  if (!body.entries || typeof body.entries !== "object") {
    return Response.json({ error: "invalid body: entries required" }, { status: 400 });
  }

  // Per-entry size check
  for (const [key, content] of Object.entries(body.entries)) {
    if (typeof content !== "string") {
      return Response.json({ error: `invalid entry: ${key} must be string` }, { status: 400 });
    }
    if (new TextEncoder().encode(content).byteLength > MAX_ENTRY_SIZE) {
      return Response.json({ error: `entry too large: ${key}` }, { status: 413 });
    }
  }

  // Capacity check
  const currentCount = db.getEntryCount(repo);
  const newKeys = Object.keys(body.entries).filter((k) => !db.hasEntry(repo, k));
  if (currentCount + newKeys.length > MAX_ENTRIES) {
    return Response.json(
      { error: "too_many_entries", extra_details: { max_entries: MAX_ENTRIES } },
      { status: 413 }
    );
  }

  const newChecksum = db.upsertEntries(repo, body.entries);
  return Response.json({ checksum: newChecksum }, { headers: { ETag: newChecksum } });
}
