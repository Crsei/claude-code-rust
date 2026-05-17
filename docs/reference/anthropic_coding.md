# Anthropic-Compatible Coding API

Use `ANTHROPIC_BASE_URL` to route the Anthropic Messages API client to an
Anthropic-compatible endpoint. In compatible mode, configure provider-native
model IDs explicitly; cc-rust will not fall back to official Claude model IDs
for third-party endpoints.

## Direct Anthropic

```json
{
  "env": {
    "ANTHROPIC_API_KEY": "sk-ant-***",
    "ANTHROPIC_MODEL": "MOTA"
  }
}
```

Direct Anthropic resolves neutral aliases to official defaults when no
provider-specific override is set:

| Alias | Env override | Direct Anthropic fallback |
| --- | --- | --- |
| `SOTA` | `ANTHROPIC_DEFAULT_SOTA_MODEL` | `claude-opus-4-7` |
| `MOTA` | `ANTHROPIC_DEFAULT_MOTA_MODEL` | `claude-sonnet-4-6` |
| `FOTA` | `ANTHROPIC_DEFAULT_FOTA_MODEL` | `claude-haiku-4-5-20251001` |

Legacy fallback env vars are still read only when the new env var is unset:
`ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, and
`ANTHROPIC_DEFAULT_HAIKU_MODEL`. Their use emits a warning without printing
the configured model value.

## Compatible Anthropic

When `ANTHROPIC_BASE_URL` points away from `https://api.anthropic.com`, aliases
must resolve through provider-specific env vars, or `ANTHROPIC_MODEL` must be a
full provider model ID.

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "sk-***",
    "ANTHROPIC_BASE_URL": "https://api.example.com/anthropic",
    "ANTHROPIC_MODEL": "MOTA",
    "ANTHROPIC_DEFAULT_SOTA_MODEL": "provider-coding-pro",
    "ANTHROPIC_DEFAULT_MOTA_MODEL": "provider-coding",
    "ANTHROPIC_DEFAULT_FOTA_MODEL": "provider-coding-fast"
  }
}
```

Explicit model IDs always win over alias defaults:

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "sk-***",
    "ANTHROPIC_BASE_URL": "https://api.example.com/anthropic",
    "ANTHROPIC_MODEL": "provider-coding"
  }
}
```
