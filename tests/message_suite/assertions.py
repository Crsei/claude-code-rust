"""Assertion implementations for the message-test suite.

Each assertion takes the full execution Context and a params dict (whatever
was in the yaml), and returns an AssertOutcome describing pass/fail plus a
human-readable ``observed`` summary so CI reports stay actionable.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Callable

from protocol import ChatResult, SseEvent


# ---------------------------------------------------------------------------
# Types
# ---------------------------------------------------------------------------


@dataclass
class AssertOutcome:
    kind: str
    passed: bool
    message: str
    observed: Any = None


@dataclass
class StepRecord:
    """Shared view exposed to assertions for one step's output."""

    kind: str  # "chat" | "command" | "setting" | "http" | "wait_ms"
    chat: ChatResult | None = None
    command_response: dict | None = None
    state_after: dict | None = None
    http_response: Any = None  # last GET /api/... JSON body


# ---------------------------------------------------------------------------
# Registry
# ---------------------------------------------------------------------------


AssertFn = Callable[[StepRecord, dict, dict], AssertOutcome]
# params = the yaml object; globals_ = shared across assertions (e.g. workspace)

_REGISTRY: dict[str, AssertFn] = {}


def register(name: str) -> Callable[[AssertFn], AssertFn]:
    def deco(fn: AssertFn) -> AssertFn:
        _REGISTRY[name] = fn
        return fn

    return deco


def run_assertion(
    name: str, step: StepRecord, params: dict, globals_: dict
) -> AssertOutcome:
    fn = _REGISTRY.get(name)
    if fn is None:
        return AssertOutcome(
            kind=name, passed=False, message=f"unknown assertion kind: {name}"
        )
    try:
        return fn(step, params, globals_)
    except Exception as e:  # noqa: BLE001
        return AssertOutcome(
            kind=name, passed=False, message=f"assertion raised: {e!r}"
        )


# ---------------------------------------------------------------------------
# Assertions — chat text
# ---------------------------------------------------------------------------


@register("response_contains")
def _response_contains(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    needles = params if isinstance(params, list) else params.get("substrings") or []
    text = _gather_text(step)
    missing = [n for n in needles if n not in text]
    return AssertOutcome(
        kind="response_contains",
        passed=not missing,
        message=(
            "all substrings found" if not missing else f"missing: {missing!r}"
        ),
        observed=_trunc(text),
    )


@register("response_matches")
def _response_matches(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    pattern = params if isinstance(params, str) else params.get("pattern")
    text = _gather_text(step)
    ok = bool(re.search(pattern, text, re.MULTILINE | re.DOTALL))
    return AssertOutcome(
        kind="response_matches",
        passed=ok,
        message=f"pattern /{pattern}/ {'matched' if ok else 'not found'}",
        observed=_trunc(text),
    )


# ---------------------------------------------------------------------------
# Assertions — SSE stream
# ---------------------------------------------------------------------------


@register("sse_event")
def _sse_event(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if not step.chat:
        return AssertOutcome(
            kind="sse_event", passed=False, message="step has no chat result"
        )
    want_type = params.get("type")
    match = params.get("match") or {}
    hits = [e for e in step.chat.events if e.event == want_type]
    if match:
        hits = [e for e in hits if _deep_contains(e.data, match)]
    return AssertOutcome(
        kind="sse_event",
        passed=bool(hits),
        message=f"found {len(hits)} matching event(s) of type {want_type!r}",
        observed=[_describe_event(e) for e in hits[:3]],
    )


@register("sse_event_not")
def _sse_event_not(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if not step.chat:
        return AssertOutcome(
            kind="sse_event_not", passed=True, message="no chat — trivially absent"
        )
    want_type = params.get("type")
    match = params.get("match") or {}
    hits = [e for e in step.chat.events if e.event == want_type]
    if match:
        hits = [e for e in hits if _deep_contains(e.data, match)]
    return AssertOutcome(
        kind="sse_event_not",
        passed=not hits,
        message=(
            f"unexpectedly found {len(hits)} matching event(s) of type {want_type!r}"
            if hits
            else "absent as expected"
        ),
        observed=[_describe_event(e) for e in hits[:3]],
    )


@register("tool_call_order")
def _tool_call_order(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if not step.chat:
        return AssertOutcome(
            kind="tool_call_order", passed=False, message="no chat result"
        )
    want = params if isinstance(params, list) else params.get("names") or []
    names = [tu.get("name") for tu in step.chat.tool_uses]
    ok = _is_subsequence(want, names)
    return AssertOutcome(
        kind="tool_call_order",
        passed=ok,
        message=(
            f"tool sequence {names} contains subsequence {want}"
            if ok
            else f"subsequence {want} not found in {names}"
        ),
        observed=names,
    )


# ---------------------------------------------------------------------------
# Assertions — API state
# ---------------------------------------------------------------------------


@register("api_state")
def _api_state(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if step.state_after is None:
        return AssertOutcome(
            kind="api_state",
            passed=False,
            message="step did not record /api/state",
        )
    return _assert_on_json(step.state_after, params, kind="api_state")


@register("http_json")
def _http_json(step: StepRecord, params: dict, globals_: dict) -> AssertOutcome:
    """Assert against the JSON body returned by the most recent http step.

    Params:
        path: dotted jsonpath into the body (e.g. current_workspace.cwd)
        equals: exact match
        contains: substring (supports ``${workspace}`` expansion)
        matches: regex
    """
    if step.http_response is None:
        return AssertOutcome(
            kind="http_json",
            passed=False,
            message="step has no http response",
        )
    # expand ${workspace} etc. inside `contains` / `equals`
    p = dict(params)
    for k in ("contains", "equals", "matches"):
        if isinstance(p.get(k), str):
            p[k] = _expand(p[k], globals_)
    return _assert_on_json(step.http_response, p, kind="http_json")


def _assert_on_json(body: Any, params: dict, *, kind: str) -> AssertOutcome:
    path = params.get("path")
    got = _jsonpath(body, path) if path else body
    if "equals" in params:
        want = params["equals"]
        ok = got == want
        msg = f"{path!r} == {want!r}" if ok else f"{path!r}={got!r} != {want!r}"
    elif "contains" in params:
        needle = params["contains"]
        ok = needle in (got or "") if isinstance(got, str) else False
        msg = (
            f"{path!r} contains {needle!r}"
            if ok
            else f"{path!r}={got!r} lacks {needle!r}"
        )
    elif "matches" in params:
        ok = isinstance(got, str) and re.search(params["matches"], got) is not None
        msg = f"{path!r} {'matches' if ok else 'no-match'} /{params['matches']}/"
    else:
        ok = got is not None
        msg = f"{path!r} {'present' if ok else 'missing'}"
    return AssertOutcome(kind=kind, passed=ok, message=msg, observed=got)


def _expand(s: str, globals_: dict) -> str:
    out = s
    for k, v in (globals_ or {}).items():
        out = out.replace("${" + k + "}", str(v))
    return out


# ---------------------------------------------------------------------------
# Assertions — command responses
# ---------------------------------------------------------------------------


@register("command_response")
def _command_response(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if step.command_response is None:
        return AssertOutcome(
            kind="command_response",
            passed=False,
            message="step has no /api/command response",
        )
    want_type = params.get("type")
    contains = params.get("contains") or []
    if isinstance(contains, str):
        contains = [contains]
    r = step.command_response
    type_ok = want_type is None or r.get("type") == want_type
    content = r.get("content") or ""
    missing = [c for c in contains if c not in content]
    ok = type_ok and not missing
    parts = []
    if not type_ok:
        parts.append(f"type={r.get('type')} ≠ {want_type}")
    if missing:
        parts.append(f"missing substrings: {missing}")
    return AssertOutcome(
        kind="command_response",
        passed=ok,
        message="; ".join(parts) or "ok",
        observed=_trunc(content),
    )


# ---------------------------------------------------------------------------
# Assertions — filesystem
# ---------------------------------------------------------------------------


@register("fs_exists")
def _fs_exists(step: StepRecord, params: dict, globals_: dict) -> AssertOutcome:
    import os

    p = _resolve_path(params if isinstance(params, str) else params.get("path"), globals_)
    ok = os.path.exists(p)
    return AssertOutcome(
        kind="fs_exists",
        passed=ok,
        message=f"{p} {'exists' if ok else 'missing'}",
        observed=p,
    )


@register("fs_contains")
def _fs_contains(step: StepRecord, params: dict, globals_: dict) -> AssertOutcome:
    import os

    p = _resolve_path(params.get("path"), globals_)
    pattern = params.get("pattern") or params.get("substring")
    if not os.path.exists(p):
        return AssertOutcome(
            kind="fs_contains", passed=False, message=f"{p} not found", observed=p
        )
    text = open(p, "r", encoding="utf-8", errors="replace").read()
    if params.get("pattern"):
        ok = re.search(pattern, text, re.MULTILINE | re.DOTALL) is not None
    else:
        ok = pattern in text
    return AssertOutcome(
        kind="fs_contains",
        passed=ok,
        message=f"{pattern!r} {'found' if ok else 'missing'} in {p}",
        observed=_trunc(text),
    )


# ---------------------------------------------------------------------------
# Assertions — cost / timing
# ---------------------------------------------------------------------------


@register("cost_between")
def _cost_between(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if not step.chat or step.chat.cost_usd is None:
        return AssertOutcome(
            kind="cost_between", passed=False, message="no cost recorded"
        )
    lo, hi = params[0], params[1]
    c = step.chat.cost_usd
    ok = lo <= c <= hi
    return AssertOutcome(
        kind="cost_between",
        passed=ok,
        message=f"cost {c} {'within' if ok else 'outside'} [{lo},{hi}]",
        observed=c,
    )


@register("latency_under_sec")
def _latency_under(step: StepRecord, params: dict, _: dict) -> AssertOutcome:
    if not step.chat or step.chat.duration_ms is None:
        return AssertOutcome(
            kind="latency_under_sec", passed=False, message="no duration recorded"
        )
    limit_s = params if isinstance(params, (int, float)) else params.get("sec")
    secs = step.chat.duration_ms / 1000.0
    ok = secs < limit_s
    return AssertOutcome(
        kind="latency_under_sec",
        passed=ok,
        message=f"duration {secs:.2f}s {'<' if ok else '≥'} {limit_s}s",
        observed=secs,
    )


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _gather_text(step: StepRecord) -> str:
    pieces: list[str] = []
    if step.chat and step.chat.assistant_text:
        pieces.append(step.chat.assistant_text)
    if step.command_response:
        c = step.command_response.get("content")
        if c:
            pieces.append(c)
    return "\n".join(pieces)


def _describe_event(e: SseEvent) -> dict:
    # Keep it short but diagnostic — dump first ~200 chars of data.
    raw = e.raw
    if len(raw) > 200:
        raw = raw[:200] + "…"
    return {"event": e.event, "data": raw}


def _deep_contains(haystack: Any, needle: Any) -> bool:
    """True if every key in needle appears under the same path in haystack
    with matching value. Strings inside needle are substring-matched; nested
    dicts recurse; other values are compared with ==."""
    if isinstance(needle, dict):
        if not isinstance(haystack, dict):
            return False
        return all(
            k in haystack and _deep_contains(haystack[k], v) for k, v in needle.items()
        )
    if isinstance(needle, list):
        if not isinstance(haystack, list):
            return False
        return all(any(_deep_contains(h, n) for h in haystack) for n in needle)
    if isinstance(needle, str) and isinstance(haystack, str):
        return needle in haystack
    return haystack == needle


def _is_subsequence(want: list, seq: list) -> bool:
    it = iter(seq)
    return all(w in it for w in want)


def _jsonpath(obj: Any, path: str) -> Any:
    """Minimal dotted-path accessor. Supports ``a.b[0].c`` style."""
    import re as _re

    cur = obj
    for part in _re.findall(r"[^.\[\]]+|\[\d+\]", path or ""):
        if part.startswith("["):
            cur = cur[int(part[1:-1])]
        else:
            cur = cur.get(part) if isinstance(cur, dict) else getattr(cur, part, None)
        if cur is None:
            return None
    return cur


def _resolve_path(p: str | None, globals_: dict) -> str:
    """Expand ``${workspace}`` and ``${home}`` placeholders."""
    if p is None:
        return ""
    out = p
    for k, v in (globals_ or {}).items():
        out = out.replace("${" + k + "}", str(v))
    return out


def _trunc(s: str, n: int = 400) -> str:
    if s is None:
        return ""
    return s if len(s) <= n else s[:n] + "…"


__all__ = ["AssertOutcome", "StepRecord", "run_assertion"]
