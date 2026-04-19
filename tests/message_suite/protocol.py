"""HTTP + SSE client for the cc-rust web backend.

Thin async wrapper around httpx targeted at the message-test suite. Keeps
everything text/JSON — no browser involvement.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from typing import Any, AsyncIterator

import httpx


@dataclass
class SseEvent:
    """A single parsed SSE event."""

    event: str
    data: Any  # already-json-parsed; falls back to raw str if unparseable
    raw: str


@dataclass
class ChatResult:
    """Aggregated outcome of a single /api/chat round-trip."""

    events: list[SseEvent] = field(default_factory=list)
    assistant_text: str = ""
    tool_uses: list[dict] = field(default_factory=list)
    tool_results: list[dict] = field(default_factory=list)
    stop_reason: str | None = None
    usage: dict | None = None
    cost_usd: float | None = None
    duration_ms: int | None = None
    error: str | None = None


class WebClient:
    """High-level client over the cc-rust web API."""

    def __init__(self, base_url: str, *, default_timeout: float = 60.0):
        self.base_url = base_url.rstrip("/")
        self.default_timeout = default_timeout
        self._client = httpx.AsyncClient(timeout=default_timeout)

    async def close(self) -> None:
        await self._client.aclose()

    # ------------------------------------------------------------------ state
    async def get_state(self) -> dict:
        r = await self._client.get(f"{self.base_url}/api/state")
        r.raise_for_status()
        return r.json()

    async def get_json(self, path: str) -> Any:
        """Arbitrary GET returning parsed JSON. For yaml ``http`` steps."""
        if not path.startswith("/"):
            path = "/" + path
        r = await self._client.get(f"{self.base_url}{path}")
        r.raise_for_status()
        return r.json()

    async def wait_ready(self, deadline_sec: float = 30.0) -> dict:
        """Poll /api/state until it responds 200 or the deadline elapses."""
        import asyncio
        import time

        start = time.monotonic()
        last_exc: Exception | None = None
        while time.monotonic() - start < deadline_sec:
            try:
                return await self.get_state()
            except Exception as e:  # noqa: BLE001 — we want broad retry
                last_exc = e
                await asyncio.sleep(0.2)
        raise RuntimeError(
            f"server at {self.base_url} not ready after {deadline_sec}s: {last_exc}"
        )

    # ---------------------------------------------------------------- command
    async def run_command(self, command: str, args: str = "") -> dict:
        """POST /api/command. Returns {type, content}."""
        r = await self._client.post(
            f"{self.base_url}/api/command",
            json={"command": command, "args": args},
        )
        r.raise_for_status()
        return r.json()

    # ---------------------------------------------------------------- setting
    async def set(self, action: str, value: Any) -> dict:
        r = await self._client.post(
            f"{self.base_url}/api/settings",
            json={"action": action, "value": value},
        )
        r.raise_for_status()
        return r.json()

    # ------------------------------------------------------------------ chat
    async def chat(
        self,
        message: str,
        *,
        session_id: str | None = None,
        timeout_sec: float = 120.0,
    ) -> ChatResult:
        """Send a chat message and collect the SSE stream into a ChatResult."""

        result = ChatResult()
        payload: dict[str, Any] = {"message": message}
        if session_id:
            payload["session_id"] = session_id

        async with self._client.stream(
            "POST",
            f"{self.base_url}/api/chat",
            json=payload,
            timeout=timeout_sec,
        ) as resp:
            if resp.status_code != 200:
                body = await resp.aread()
                result.error = f"HTTP {resp.status_code}: {body.decode('utf-8', 'replace')}"
                return result

            async for evt in _iter_sse(resp):
                result.events.append(evt)
                _fold_event(evt, result)

        return result

    async def abort(self, session_id: str | None = None) -> None:
        await self._client.post(
            f"{self.base_url}/api/abort",
            json={"session_id": session_id} if session_id else {},
        )

    # ----------------------------------------------------------------- sessions
    async def list_sessions(self) -> dict:
        r = await self._client.get(f"{self.base_url}/api/sessions")
        r.raise_for_status()
        return r.json()

    async def new_session(self) -> dict:
        r = await self._client.post(f"{self.base_url}/api/sessions/new")
        r.raise_for_status()
        return r.json()


# ---------------------------------------------------------------------------
# SSE parsing
# ---------------------------------------------------------------------------


async def _iter_sse(resp: httpx.Response) -> AsyncIterator[SseEvent]:
    """Parse an SSE response stream into SseEvent values.

    We mirror the cc-rust serializer in ``src/web/sse.rs``: each event block
    is terminated by a blank line; ``event:`` sets the type, ``data:`` is a
    single JSON payload per event.
    """
    current_event = ""
    current_data: list[str] = []
    async for line in resp.aiter_lines():
        if line == "":
            if current_event and current_data:
                raw = "\n".join(current_data)
                try:
                    data = json.loads(raw)
                except json.JSONDecodeError:
                    data = raw
                yield SseEvent(event=current_event, data=data, raw=raw)
            current_event = ""
            current_data = []
            continue
        if line.startswith(":"):
            continue  # comment / keep-alive
        if line.startswith("event:"):
            current_event = line[len("event:") :].strip()
        elif line.startswith("data:"):
            current_data.append(line[len("data:") :].lstrip())


def _fold_event(evt: SseEvent, result: ChatResult) -> None:
    """Accumulate a single SSE event into the running ChatResult."""

    if evt.event == "stream_event":
        inner = (evt.data or {}).get("event") or {}
        t = inner.get("type")
        if t == "content_block_delta":
            delta = inner.get("delta") or {}
            if delta.get("type") == "text_delta":
                result.assistant_text += delta.get("text") or ""
    elif evt.event == "assistant":
        msg = (evt.data or {}).get("message") or evt.data or {}
        content = msg.get("content") or []
        # Text blocks — if streaming deltas already accumulated this, we
        # overwrite with the authoritative final text.
        text = "".join(b.get("text") or "" for b in content if b.get("type") == "text")
        if text:
            result.assistant_text = text
        for b in content:
            if b.get("type") == "tool_use":
                result.tool_uses.append(b)
        if msg.get("usage"):
            result.usage = msg["usage"]
        if msg.get("cost_usd") is not None:
            result.cost_usd = msg["cost_usd"]
    elif evt.event == "user_replay":
        blocks = (evt.data or {}).get("content_blocks") or (evt.data or {}).get(
            "content"
        ) or []
        for b in blocks:
            if b.get("type") == "tool_result":
                result.tool_results.append(b)
    elif evt.event == "result":
        d = evt.data or {}
        result.stop_reason = d.get("stop_reason")
        if d.get("duration_ms") is not None:
            result.duration_ms = d["duration_ms"]
        if result.cost_usd is None and d.get("cost_usd") is not None:
            result.cost_usd = d["cost_usd"]


__all__ = ["WebClient", "ChatResult", "SseEvent"]


# ---------------------------------------------------------------------------
# Convenience helpers used by assertions
# ---------------------------------------------------------------------------


def regex_match(pattern: str, text: str) -> bool:
    return re.search(pattern, text, re.MULTILINE) is not None
