#!/usr/bin/env python3
"""Provider smoke matrix for Anthropic-compatible coding mode.

Default mode runs mock/unit smoke checks only. Live provider checks are
credential-gated and never print configured secret values or raw auth headers.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
LOCAL_CARGO_BIN = REPO_ROOT.parent / ".rust" / "cargo" / "bin"
LOCAL_RUSTUP_HOME = REPO_ROOT.parent / ".rust" / "rustup"


SECRET_ENV_KEYS = {
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "CLAUDE_CODE_VERTEX_ACCESS_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS_JSON",
}


@dataclass(frozen=True)
class MockCase:
    name: str
    command: list[str]


@dataclass(frozen=True)
class RealCase:
    name: str
    required_any: tuple[tuple[str, ...], ...]
    env: dict[str, str]
    remove_env: tuple[str, ...] = ()


MOCK_CASES = [
    MockCase(
        "direct-anthropic-api-key",
        ["cargo", "test", "-p", "cc-api", "test_from_env_with_anthropic_key"],
    ),
    MockCase(
        "direct-anthropic-bearer",
        [
            "cargo",
            "test",
            "-p",
            "cc-api",
            "regression_anthropic_auth_token_uses_authorization_bearer",
        ],
    ),
    MockCase(
        "compatible-anthropic-bearer-custom-base-url",
        [
            "cargo",
            "test",
            "-p",
            "cc-api",
            "anthropic_auth_token_with_non_official_base_url_selects_compatible_messages_endpoint",
        ],
    ),
    MockCase(
        "bedrock-model-mapping",
        ["cargo", "test", "-p", "cc-models", "first_party_to_bedrock_sonnet45"],
    ),
    MockCase(
        "vertex-model-mapping",
        ["cargo", "test", "-p", "cc-models", "first_party_to_vertex_sonnet45"],
    ),
    MockCase(
        "prompt-cache-disabled-for-non-anthropic-fields",
        ["cargo", "test", "-p", "cc-api", "test_strip_anthropic_cache_fields_recursively"],
    ),
    MockCase(
        "prompt-cache-enabled-ttl-global-gated",
        [
            "cargo",
            "test",
            "-p",
            "cc-api",
            "test_prompt_cache_policy_adds_ttl_and_global_only_when_capable",
        ],
    ),
]


REAL_CASES = [
    RealCase(
        "direct-anthropic-api-key",
        required_any=(("ANTHROPIC_API_KEY",),),
        remove_env=("ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"),
        env={"ANTHROPIC_MODEL": "MOTA"},
    ),
    RealCase(
        "direct-anthropic-bearer",
        required_any=(("ANTHROPIC_AUTH_TOKEN",),),
        remove_env=("ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL"),
        env={"ANTHROPIC_MODEL": "MOTA"},
    ),
    RealCase(
        "compatible-anthropic-bearer-custom-base-url",
        required_any=(("ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL"),),
        remove_env=("ANTHROPIC_API_KEY",),
        env={},
    ),
    RealCase(
        "bedrock-model-mapping",
        required_any=(
            ("AWS_BEARER_TOKEN_BEDROCK",),
            ("AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"),
        ),
        env={
            "CLAUDE_CODE_USE_BEDROCK": "1",
            "ANTHROPIC_MODEL": os.environ.get(
                "ANTHROPIC_MODEL", "claude-sonnet-4-5-20250929"
            ),
        },
        remove_env=("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"),
    ),
    RealCase(
        "vertex-model-mapping",
        required_any=(
            ("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", "GOOGLE_CLOUD_PROJECT"),
            ("GOOGLE_APPLICATION_CREDENTIALS", "GOOGLE_CLOUD_PROJECT"),
            ("GOOGLE_APPLICATION_CREDENTIALS_JSON", "GOOGLE_CLOUD_PROJECT"),
        ),
        env={
            "CLAUDE_CODE_USE_VERTEX": "1",
            "ANTHROPIC_MODEL": os.environ.get(
                "ANTHROPIC_MODEL", "claude-sonnet-4-5-20250929"
            ),
        },
        remove_env=("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"),
    ),
    RealCase(
        "prompt-cache-default-enabled",
        required_any=(("ANTHROPIC_API_KEY",), ("ANTHROPIC_AUTH_TOKEN",)),
        env={"ANTHROPIC_MODEL": "MOTA"},
        remove_env=(
            "ANTHROPIC_BASE_URL",
            "CC_RUST_PROMPT_CACHE_TTL",
            "CC_RUST_PROMPT_CACHE_GLOBAL",
        ),
    ),
    RealCase(
        "prompt-cache-ttl-global-enabled",
        required_any=(("ANTHROPIC_API_KEY",), ("ANTHROPIC_AUTH_TOKEN",)),
        env={
            "ANTHROPIC_MODEL": "MOTA",
            "CC_RUST_PROMPT_CACHE_TTL": "1h",
            "CC_RUST_PROMPT_CACHE_GLOBAL": "1",
        },
        remove_env=("ANTHROPIC_BASE_URL",),
    ),
]


def cargo_env(base: dict[str, str]) -> dict[str, str]:
    env = dict(base)
    if LOCAL_CARGO_BIN.exists():
        env["CARGO_HOME"] = str(REPO_ROOT.parent / ".rust" / "cargo")
        env["RUSTUP_HOME"] = str(LOCAL_RUSTUP_HOME)
        env["PATH"] = f"{LOCAL_CARGO_BIN}{os.pathsep}{env.get('PATH', '')}"
    return env


def redactor(env: dict[str, str]):
    secrets = [
        value
        for key, value in env.items()
        if key in SECRET_ENV_KEYS and value and len(value) >= 6
    ]

    def redact(text: str) -> str:
        redacted = text
        for secret in secrets:
            redacted = redacted.replace(secret, "<redacted>")
        redacted = re.sub(
            r"(?i)(authorization\s*:\s*bearer\s+)[^\s,;]+",
            r"\1<redacted>",
            redacted,
        )
        redacted = re.sub(
            r"(?i)(x-api-key\s*:\s*)[^\s,;]+",
            r"\1<redacted>",
            redacted,
        )
        return redacted

    return redact


def run_command(name: str, command: list[str], env: dict[str, str]) -> int:
    printable = " ".join(command)
    print(f"[run] {name}: {printable}")
    proc = subprocess.run(
        command,
        cwd=REPO_ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    output = redactor(env)(proc.stdout)
    if proc.returncode != 0:
        print(output[-12000:])
    print(f"[{'ok' if proc.returncode == 0 else 'fail'}] {name}")
    return proc.returncode


def requirements_present(case: RealCase, env: dict[str, str]) -> bool:
    return any(all(env.get(key) for key in group) for group in case.required_any)


def real_command() -> list[str]:
    prompt = "Reply with exactly OK for the provider smoke check."
    return [
        "cargo",
        "run",
        "-q",
        "-p",
        "claude-code-rs",
        "--",
        "-p",
        "--max-turns",
        "1",
        "--output-format",
        "text",
        prompt,
    ]


def run_mock(base_env: dict[str, str]) -> int:
    env = cargo_env(base_env)
    status = 0
    for case in MOCK_CASES:
        status = run_command(case.name, case.command, env) or status
    return status


def run_real(base_env: dict[str, str]) -> int:
    status = 0
    for case in REAL_CASES:
        env = cargo_env(base_env)
        for key in case.remove_env:
            env.pop(key, None)
        env.update(case.env)
        if not requirements_present(case, env):
            groups = ["+".join(group) for group in case.required_any]
            print(f"[skip] {case.name}: missing one of {', '.join(groups)}")
            continue
        with tempfile.TemporaryDirectory(prefix="cc-rust-provider-smoke-") as home:
            env["CC_RUST_HOME"] = home
            status = run_command(case.name, real_command(), env) or status
    return status


def list_cases() -> int:
    print("mock:")
    for case in MOCK_CASES:
        print(f"  - {case.name}")
    print("real:")
    for case in REAL_CASES:
        groups = ["+".join(group) for group in case.required_any]
        print(f"  - {case.name} ({' or '.join(groups)})")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mode",
        choices=("list", "mock", "real", "all"),
        nargs="?",
        default="mock",
        help="Smoke set to run. Default: mock.",
    )
    args = parser.parse_args()

    if args.mode == "list":
        return list_cases()

    status = 0
    if args.mode in ("mock", "all"):
        status = run_mock(os.environ) or status
    if args.mode in ("real", "all"):
        status = run_real(os.environ) or status
    return status


if __name__ == "__main__":
    sys.exit(main())
