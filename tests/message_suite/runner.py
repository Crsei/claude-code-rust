"""Message-test suite runner.

Design goals:
    • Per-case subprocess isolation (own port + own CC_RUST_HOME + own cwd)
    • Text-only transport — HTTP + SSE, no browser
    • Fan-out across concurrency groups; serial within a group
    • Human + JSONL traces under ``results/<timestamp>/<case-id>.*``

Invoke from repo root:
    python tests/message_suite/runner.py                 # run every case
    python tests/message_suite/runner.py -k smoke.*      # filter by id glob
    python tests/message_suite/runner.py --parallel 4    # max concurrent cases
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
import fnmatch
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import traceback
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

import yaml

# Local imports — when runner is invoked via ``python tests/message_suite/runner.py``
# the suite folder is *not* on sys.path, so add it explicitly.
SUITE_DIR = Path(__file__).resolve().parent
if str(SUITE_DIR) not in sys.path:
    sys.path.insert(0, str(SUITE_DIR))

from assertions import AssertOutcome, StepRecord, run_assertion  # noqa: E402
from protocol import WebClient  # noqa: E402


REPO_ROOT = SUITE_DIR.parent.parent  # rust/
DEFAULT_BINARY = REPO_ROOT / "target" / "debug" / "claude-code-rs.exe"

# Make sure unicode icons print on Windows GBK consoles
if hasattr(sys.stdout, "reconfigure"):
    with contextlib.suppress(Exception):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]


# ---------------------------------------------------------------------------
# Case parsing
# ---------------------------------------------------------------------------


@dataclass
class Case:
    id: str
    path: Path
    category: str
    concurrency_group: str
    timeout_sec: float
    prereq: dict
    steps: list[dict]
    cleanup: dict
    raw: dict


def load_defaults(suite_dir: Path) -> dict:
    p = suite_dir / "defaults.yaml"
    if not p.exists():
        return {}
    return yaml.safe_load(p.read_text(encoding="utf-8")) or {}


def _deep_merge(base: dict, override: dict) -> dict:
    """Recursive dict merge; ``override`` wins at leaves."""
    out = dict(base)
    for k, v in (override or {}).items():
        if k in out and isinstance(out[k], dict) and isinstance(v, dict):
            out[k] = _deep_merge(out[k], v)
        else:
            out[k] = v
    return out


def load_cases(root: Path, defaults: dict | None = None) -> list[Case]:
    cases: list[Case] = []
    defaults = defaults or {}
    for p in sorted(root.rglob("*.yaml")):
        with p.open("r", encoding="utf-8") as f:
            raw = yaml.safe_load(f) or {}
        merged_prereq = _deep_merge(defaults.get("prereq") or {}, raw.get("prereq") or {})
        cases.append(
            Case(
                id=raw.get("id") or p.stem,
                path=p,
                category=raw.get("category") or p.parent.name,
                concurrency_group=raw.get("concurrency_group") or raw.get("category") or p.parent.name,
                timeout_sec=float(raw.get("timeout_sec", 120)),
                prereq=merged_prereq,
                steps=raw.get("steps") or [],
                cleanup=raw.get("cleanup") or {},
                raw=raw,
            )
        )
    return cases


def filter_cases(cases: list[Case], patterns: list[str]) -> list[Case]:
    if not patterns:
        return cases
    out = []
    for c in cases:
        if any(fnmatch.fnmatchcase(c.id, p) for p in patterns):
            out.append(c)
    return out


# ---------------------------------------------------------------------------
# Port allocation
# ---------------------------------------------------------------------------


def pick_free_port(start: int = 13000, end: int = 13900) -> int:
    """Bind-and-release to find a free localhost port in a range."""
    for p in range(start, end):
        with contextlib.closing(socket.socket(socket.AF_INET, socket.SOCK_STREAM)) as s:
            try:
                s.bind(("127.0.0.1", p))
                return p
            except OSError:
                continue
    raise RuntimeError(f"no free port in [{start},{end})")


# ---------------------------------------------------------------------------
# Subprocess lifecycle
# ---------------------------------------------------------------------------


@dataclass
class ServerHandle:
    process: subprocess.Popen
    port: int
    cwd: Path
    home: Path
    stdout_log: Path
    stderr_log: Path

    def kill(self, timeout: float = 5.0) -> None:
        if self.process.poll() is not None:
            return
        try:
            if os.name == "nt":
                # Send CTRL_BREAK for graceful shutdown; fall back to kill.
                try:
                    self.process.send_signal(signal.CTRL_BREAK_EVENT)
                except (AttributeError, ValueError):
                    self.process.terminate()
            else:
                self.process.terminate()
            self.process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            self.process.kill()
            with contextlib.suppress(Exception):
                self.process.wait(timeout=2)


def spawn_server(
    binary: Path,
    *,
    cwd: Path,
    home: Path,
    port: int,
    env_overrides: dict[str, str],
    log_dir: Path,
    case_id: str,
) -> ServerHandle:
    env = os.environ.copy()
    env["CC_RUST_HOME"] = str(home)
    env["RUST_LOG"] = env.get("RUST_LOG", "warn,claude_code_rs=info")
    env.update(env_overrides)

    stdout_log = log_dir / f"{case_id}.stdout.log"
    stderr_log = log_dir / f"{case_id}.stderr.log"
    stdout_log.parent.mkdir(parents=True, exist_ok=True)

    args = [
        str(binary),
        "--web",
        "--web-port",
        str(port),
        "--no-open",
        "--cwd",
        str(cwd),
    ]

    creationflags = 0
    if os.name == "nt":
        creationflags = subprocess.CREATE_NEW_PROCESS_GROUP

    proc = subprocess.Popen(
        args,
        env=env,
        cwd=str(cwd),
        stdout=open(stdout_log, "w", encoding="utf-8"),
        stderr=open(stderr_log, "w", encoding="utf-8"),
        creationflags=creationflags,
    )
    return ServerHandle(
        process=proc,
        port=port,
        cwd=cwd,
        home=home,
        stdout_log=stdout_log,
        stderr_log=stderr_log,
    )


# ---------------------------------------------------------------------------
# Case execution
# ---------------------------------------------------------------------------


@dataclass
class StepReport:
    index: int
    kind: str
    name: str
    duration_ms: int
    record: StepRecord | None
    assertions: list[AssertOutcome] = field(default_factory=list)
    error: str | None = None


@dataclass
class CaseReport:
    case_id: str
    status: str  # "passed" | "failed" | "errored"
    duration_ms: int
    steps: list[StepReport] = field(default_factory=list)
    error: str | None = None
    server_stderr_tail: str = ""


class CaseRunner:
    def __init__(
        self,
        case: Case,
        binary: Path,
        run_dir: Path,
    ):
        self.case = case
        self.binary = binary
        self.run_dir = run_dir
        self.trace_path = run_dir / f"{case.id}.jsonl"
        self.trace_path.parent.mkdir(parents=True, exist_ok=True)
        self.trace = open(self.trace_path, "w", encoding="utf-8")
        self.t0 = time.monotonic()

    # ------------------------------------------------------------------
    def _log(self, kind: str, **fields) -> None:
        rec = {
            "t": round(time.monotonic() - self.t0, 4),
            "type": kind,
            **fields,
        }
        self.trace.write(json.dumps(rec, ensure_ascii=False, default=str) + "\n")
        self.trace.flush()

    # ------------------------------------------------------------------
    async def run(self) -> CaseReport:
        start = time.monotonic()
        report = CaseReport(case_id=self.case.id, status="passed", duration_ms=0)
        server: ServerHandle | None = None
        workspace: Path | None = None
        home: Path | None = None
        client: WebClient | None = None

        try:
            workspace, home = self._prepare_workspace()
            self._log("prepare_done", workspace=str(workspace), home=str(home))

            port = pick_free_port()
            env_overrides = dict(self.case.prereq.get("env") or {})
            server = spawn_server(
                self.binary,
                cwd=workspace,
                home=home,
                port=port,
                env_overrides=env_overrides,
                log_dir=self.run_dir,
                case_id=self.case.id,
            )
            self._log("server_spawned", port=port, pid=server.process.pid)

            client = WebClient(f"http://127.0.0.1:{port}")
            ready_state = await client.wait_ready(deadline_sec=30.0)
            self._log("server_ready", state=_summarize_state(ready_state))

            # Apply prereq settings (permission_mode, etc.)
            await self._apply_prereq(client)

            # Per-step execution — exposed to assertions via globals_
            self._globals = {
                "workspace": str(workspace),
                "home": str(home),
                "base_url": client.base_url,
            }
            for idx, step in enumerate(self.case.steps):
                sr = await self._run_step(client, idx, step)
                report.steps.append(sr)
                if any(not a.passed for a in sr.assertions) or sr.error:
                    report.status = "failed"
                    # keep running subsequent steps? default: stop.
                    if not self.case.raw.get("continue_on_failure"):
                        break
        except Exception as e:
            report.status = "errored"
            report.error = f"{e!r}\n{traceback.format_exc()}"
            self._log("run_exception", error=report.error)
        finally:
            if client is not None:
                with contextlib.suppress(Exception):
                    await client.close()
            if server is not None:
                server.kill()
                # Capture last ~40 lines of stderr so CI reports are useful
                try:
                    if server.stderr_log.exists():
                        tail = server.stderr_log.read_text(
                            encoding="utf-8", errors="replace"
                        ).splitlines()[-40:]
                        report.server_stderr_tail = "\n".join(tail)
                except Exception:
                    pass
            self._cleanup(workspace, home)
            report.duration_ms = int((time.monotonic() - start) * 1000)
            # Emit run_end *before* closing trace.
            self._log("run_end", status=report.status, duration_ms=report.duration_ms)
            self.trace.close()

        return report

    # ------------------------------------------------------------------
    def _prepare_workspace(self) -> tuple[Path, Path]:
        prereq = self.case.prereq or {}
        ws_raw = prereq.get("workspace")
        if ws_raw:
            ws_path = (REPO_ROOT / ws_raw).resolve()
            ws_path.mkdir(parents=True, exist_ok=True)
        else:
            ws_path = Path(tempfile.mkdtemp(prefix=f"ccrs-ws-{self.case.id}-"))

        # Always use a dedicated CC_RUST_HOME so sessions/credentials don't leak
        home_path = Path(tempfile.mkdtemp(prefix=f"ccrs-home-{self.case.id}-"))

        # Write prereq files (relative to workspace)
        for spec in prereq.get("files") or []:
            p = ws_path / spec["path"]
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(spec.get("content") or "", encoding="utf-8")

        return ws_path, home_path

    def _cleanup(self, workspace: Path | None, home: Path | None) -> None:
        # Only nuke tempdirs we created — keep fixtures and user-supplied
        # workspaces intact.
        if home and str(home).startswith(tempfile.gettempdir()):
            with contextlib.suppress(Exception):
                shutil.rmtree(home, ignore_errors=True)
        if (
            workspace
            and str(workspace).startswith(tempfile.gettempdir())
            and not self.case.prereq.get("workspace")
        ):
            with contextlib.suppress(Exception):
                shutil.rmtree(workspace, ignore_errors=True)
        # Remove files declared by cleanup.files_rm (inside workspace)
        for rel in self.case.cleanup.get("files_rm") or []:
            if workspace:
                p = workspace / rel
                with contextlib.suppress(Exception):
                    if p.is_file():
                        p.unlink()
                    elif p.is_dir():
                        shutil.rmtree(p, ignore_errors=True)

    # ------------------------------------------------------------------
    async def _apply_prereq(self, client: WebClient) -> None:
        prereq = self.case.prereq or {}
        if "permission_mode" in prereq:
            await client.set("set_permission_mode", prereq["permission_mode"])
        if "model" in prereq:
            await client.set("set_model", prereq["model"])

    # ------------------------------------------------------------------
    async def _run_step(self, client: WebClient, idx: int, step: dict) -> StepReport:
        kind = step.get("kind") or "chat"
        name = step.get("name") or f"{kind}#{idx}"
        start = time.monotonic()
        record = StepRecord(kind=kind)
        err: str | None = None

        # Expand ${workspace} / ${home} / ${base_url} in every string field.
        step = _expand_vars(step, getattr(self, "_globals", {}) or {})

        try:
            if kind == "chat":
                record.chat = await client.chat(
                    step["message"],
                    session_id=step.get("session_id"),
                    timeout_sec=float(step.get("timeout_sec", self.case.timeout_sec)),
                )
                self._log(
                    "chat_done",
                    step=idx,
                    assistant_text=record.chat.assistant_text[:200],
                    tool_uses=[tu.get("name") for tu in record.chat.tool_uses],
                    events=len(record.chat.events),
                    cost=record.chat.cost_usd,
                )
            elif kind == "command":
                record.command_response = await client.run_command(
                    step["cmd"].lstrip("/"), step.get("args", "")
                )
                self._log("command_done", step=idx, response=record.command_response)
            elif kind == "setting":
                await client.set(step["action"], step.get("value"))
                self._log("setting_done", step=idx, action=step["action"])
            elif kind == "http":
                record.http_response = await client.get_json(step["path"])
                self._log(
                    "http_done",
                    step=idx,
                    path=step["path"],
                    body_preview=json.dumps(record.http_response, default=str)[:400],
                )
            elif kind == "wait_ms":
                await asyncio.sleep(float(step.get("ms", 500)) / 1000.0)
                self._log("wait_done", step=idx)
            else:
                err = f"unknown step kind: {kind}"

            # Fresh /api/state snapshot after every step (cheap)
            try:
                record.state_after = await client.get_state()
            except Exception as e:  # noqa: BLE001
                record.state_after = None
                self._log("state_fetch_failed", step=idx, error=repr(e))
        except Exception as e:  # noqa: BLE001
            err = f"{e!r}"
            self._log("step_exception", step=idx, error=err, trace=traceback.format_exc())

        # Run assertions
        globals_ = getattr(self, "_globals", {}) or {
            "workspace": "",
            "home": os.environ.get("CC_RUST_HOME", ""),
        }
        outcomes: list[AssertOutcome] = []
        for ass in step.get("expect") or []:
            # Each ass is a single-key dict, e.g. {response_contains: [...]}.
            if isinstance(ass, dict) and len(ass) == 1:
                (k, v), = ass.items()
            else:
                k, v = ass, {}
            outcome = run_assertion(k, record, v, globals_)
            outcomes.append(outcome)
            self._log(
                "assert",
                step=idx,
                assertion=outcome.kind,
                passed=outcome.passed,
                message=outcome.message,
                observed=outcome.observed,
            )

        return StepReport(
            index=idx,
            kind=kind,
            name=name,
            duration_ms=int((time.monotonic() - start) * 1000),
            record=record,
            assertions=outcomes,
            error=err,
        )


def _expand_vars(value, globals_: dict):
    """Recursively substitute ``${var}`` placeholders in every string leaf."""
    if isinstance(value, str):
        out = value
        for k, v in globals_.items():
            out = out.replace("${" + k + "}", str(v))
        return out
    if isinstance(value, list):
        return [_expand_vars(v, globals_) for v in value]
    if isinstance(value, dict):
        return {k: _expand_vars(v, globals_) for k, v in value.items()}
    return value


def _summarize_state(state: dict) -> dict:
    return {
        "model": state.get("model"),
        "permission_mode": state.get("permission_mode"),
        "session_id": state.get("session_id"),
        "tools_count": len(state.get("tools") or []),
    }


# ---------------------------------------------------------------------------
# Orchestration
# ---------------------------------------------------------------------------


async def run_all(
    cases: list[Case], binary: Path, run_dir: Path, parallel: int
) -> list[CaseReport]:
    # Partition by concurrency_group — one semaphore per group enforces serial
    # execution within the group, and a global semaphore caps parallelism.
    global_sem = asyncio.Semaphore(parallel)
    group_locks: dict[str, asyncio.Lock] = {}

    async def run_one(case: Case) -> CaseReport:
        lock = group_locks.setdefault(case.concurrency_group, asyncio.Lock())
        async with global_sem:
            async with lock:
                print(f"▶ {case.id:45} [grp={case.concurrency_group}]", flush=True)
                runner = CaseRunner(case, binary, run_dir)
                report = await runner.run()
                status_icon = {
                    "passed": "✓",
                    "failed": "✗",
                    "errored": "‼",
                }.get(report.status, "?")
                print(
                    f"{status_icon} {case.id:45} {report.status:8} {report.duration_ms}ms",
                    flush=True,
                )
                return report

    reports = await asyncio.gather(*(run_one(c) for c in cases))
    return list(reports)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def write_junit(reports: list[CaseReport], out_path: Path) -> None:
    """Emit a minimal JUnit XML report so CI collectors can grade the run.

    Each case becomes one ``<testcase>``; step-level failures fold into a
    single ``<failure>`` element with the concatenated messages.
    """
    import html
    import xml.sax.saxutils as _sax

    def esc(s: str) -> str:
        return _sax.escape(s or "")

    total = len(reports)
    failures = sum(1 for r in reports if r.status == "failed")
    errors = sum(1 for r in reports if r.status == "errored")
    total_ms = sum(r.duration_ms for r in reports)

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<testsuite name="cc-rust-message-suite" tests="{total}" '
        f'failures="{failures}" errors="{errors}" '
        f'time="{total_ms / 1000.0:.3f}">',
    ]
    for r in reports:
        lines.append(
            f'  <testcase classname="message_suite" name="{esc(r.case_id)}" '
            f'time="{r.duration_ms / 1000.0:.3f}">'
        )
        if r.status == "failed":
            detail_parts = []
            for s in r.steps:
                for a in s.assertions:
                    if not a.passed:
                        detail_parts.append(f"step#{s.index} {a.kind}: {a.message}")
                if s.error:
                    detail_parts.append(f"step#{s.index} error: {s.error}")
            detail = "\n".join(detail_parts) or "assertion failed"
            lines.append(
                f'    <failure message="{esc(detail_parts[0] if detail_parts else "failed")}">'
                f'{esc(detail)}</failure>'
            )
        elif r.status == "errored":
            lines.append(
                f'    <error message="runtime error">{esc(r.error or "")}</error>'
            )
        if r.server_stderr_tail:
            lines.append(
                f'    <system-err>{esc(r.server_stderr_tail)}</system-err>'
            )
        lines.append("  </testcase>")
    lines.append("</testsuite>")
    out_path.write_text("\n".join(lines), encoding="utf-8")


def write_summary(reports: list[CaseReport], run_dir: Path) -> None:
    summary = {
        "total": len(reports),
        "passed": sum(1 for r in reports if r.status == "passed"),
        "failed": sum(1 for r in reports if r.status == "failed"),
        "errored": sum(1 for r in reports if r.status == "errored"),
        "cases": [
            {
                "id": r.case_id,
                "status": r.status,
                "duration_ms": r.duration_ms,
                "steps": [
                    {
                        "index": s.index,
                        "kind": s.kind,
                        "duration_ms": s.duration_ms,
                        "error": s.error,
                        "assertions": [
                            {"kind": a.kind, "passed": a.passed, "message": a.message}
                            for a in s.assertions
                        ],
                    }
                    for s in r.steps
                ],
                "error": r.error,
                "server_stderr_tail": r.server_stderr_tail,
            }
            for r in reports
        ],
    }
    (run_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    # human-readable markdown
    lines = ["# Run summary", ""]
    lines.append(
        f"**{summary['passed']}/{summary['total']} passed**, "
        f"failed={summary['failed']}, errored={summary['errored']}"
    )
    for r in reports:
        lines.append("")
        icon = {"passed": "✓", "failed": "✗", "errored": "‼"}.get(r.status, "?")
        lines.append(f"## {icon} {r.case_id} — {r.status} ({r.duration_ms}ms)")
        for s in r.steps:
            lines.append(f"  - step#{s.index} {s.kind}: {s.duration_ms}ms")
            for a in s.assertions:
                tick = "✓" if a.passed else "✗"
                lines.append(f"    {tick} {a.kind}: {a.message}")
            if s.error:
                lines.append(f"    ‼ step error: {s.error}")
        if r.error:
            lines.append(f"  error: `{r.error.splitlines()[0] if r.error else ''}`")
    (run_dir / "summary.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description="cc-rust message test suite runner")
    ap.add_argument("-k", "--filter", action="append", default=[], help="case id glob")
    ap.add_argument("--parallel", type=int, default=2, help="max concurrent cases")
    ap.add_argument(
        "--binary",
        type=Path,
        default=DEFAULT_BINARY,
        help="path to claude-code-rs binary",
    )
    ap.add_argument(
        "--cases",
        type=Path,
        default=SUITE_DIR / "cases",
        help="root dir of yaml cases",
    )
    ap.add_argument(
        "--junit",
        type=Path,
        default=None,
        help="write JUnit XML here (default: <run_dir>/junit.xml)",
    )
    args = ap.parse_args()

    if not args.binary.exists():
        print(f"binary not found: {args.binary}", file=sys.stderr)
        return 2

    defaults = load_defaults(SUITE_DIR)
    cases = filter_cases(load_cases(args.cases, defaults), args.filter)
    if not cases:
        print("no cases matched filter", file=sys.stderr)
        return 1

    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    run_dir = SUITE_DIR / "results" / stamp
    run_dir.mkdir(parents=True, exist_ok=True)
    print(f"run dir: {run_dir}")

    reports = asyncio.run(run_all(cases, args.binary, run_dir, args.parallel))
    write_summary(reports, run_dir)
    write_junit(reports, args.junit or (run_dir / "junit.xml"))

    failed = sum(1 for r in reports if r.status != "passed")
    print(f"\n{len(reports) - failed}/{len(reports)} passed → {run_dir}/summary.md")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
