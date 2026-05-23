use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// JSON Schema
// ---------------------------------------------------------------------------

/// Return a JSON Schema (Draft 2020-12) describing the on-disk
/// `settings.json` shape.
///
/// The schema is hand-maintained so the repo can commit it without pulling
/// in `schemars`. Keep in sync with [`crate::settings::RawSettings`].
pub fn settings_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://cc-rust/settings.schema.json",
        "title": "cc-rust settings",
        "description": "On-disk shape of settings.json (managed/user/project/local).",
        "type": "object",
        "additionalProperties": true,
        "properties": {
            "model": { "type": "string" },
            "backend": { "type": "string", "enum": ["native", "codex"] },
            "apiProvider": { "type": "string", "enum": ["anthropic", "openai-codex", "openai"] },
            "activeAuthProfile": { "type": "string" },
            "authProfiles": {
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "additionalProperties": true,
                    "properties": {
                        "backend": { "type": "string", "enum": ["native", "codex"] },
                        "apiProvider": { "type": "string", "enum": ["anthropic", "openai-codex", "openai"] },
                        "model": { "type": "string" },
                        "availableModels": { "type": "array", "items": { "type": "string" } },
                        "baseUrl": { "type": "string" },
                        "apiKey": { "type": "string" },
                        "env": {
                            "type": "object",
                            "additionalProperties": { "type": "string" }
                        },
                        "authSource": true
                    }
                }
            },
            "theme": { "type": "string" },
            "verbose": { "type": "boolean" },
            "permissionMode": {
                "type": "string",
                "enum": ["default", "ask", "auto", "bypass", "plan", "acceptEdits", "dontAsk"]
            },
            "allowedTools": { "type": "array", "items": { "type": "string" } },
            "permissions": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "defaultMode": {
                        "type": "string",
                        "enum": ["default", "ask", "auto", "bypass", "plan", "acceptEdits", "dontAsk"]
                    },
                    "allow": { "type": "array", "items": { "type": "string" } },
                    "ask": { "type": "array", "items": { "type": "string" } },
                    "deny": { "type": "array", "items": { "type": "string" } },
                    "additionalDirectories": {
                        "type": "array", "items": { "type": "string" }
                    },
                    "enableBypassMode": { "type": "boolean" },
                    "skipDangerousModePermissionPrompt": { "type": "boolean" },
                    "enableAutoMode": { "type": "boolean" },
                    "autoMode": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": {
                            "environment": { "type": "array", "items": { "type": "string" } },
                            "allow": { "type": "array", "items": { "type": "string" } },
                            "softDeny": { "type": "array", "items": { "type": "string" } }
                        }
                    }
                }
            },
            "sandbox": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "mode": {
                        "type": "string",
                        "enum": ["read-only", "workspace", "full"]
                    },
                    "failIfUnavailable": { "type": "boolean" },
                    "allowUnsandboxedCommands": { "type": "boolean" },
                    "allowManagedReadPathsOnly": { "type": "boolean" },
                    "allowManagedDomainsOnly": { "type": "boolean" },
                    "excludedCommands": {
                        "type": "array", "items": { "type": "string" }
                    },
                    "allowedCommands": {
                        "type": "array", "items": { "type": "string" }
                    },
                    "filesystem": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": {
                            "allowRead": { "type": "array", "items": { "type": "string" } },
                            "denyRead": { "type": "array", "items": { "type": "string" } },
                            "allowWrite": { "type": "array", "items": { "type": "string" } },
                            "denyWrite": { "type": "array", "items": { "type": "string" } }
                        }
                    },
                    "network": {
                        "type": "object",
                        "additionalProperties": true,
                        "properties": {
                            "disabled": { "type": "boolean" },
                            "allowedDomains": {
                                "type": "array", "items": { "type": "string" }
                            },
                            "httpProxyPort": { "type": "integer", "minimum": 0, "maximum": 65535 },
                            "socksProxyPort": { "type": "integer", "minimum": 0, "maximum": 65535 }
                        }
                    }
                }
            },
            "hooks": { "type": "object", "additionalProperties": true },
            "statusLine": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["none", "minimal", "command", "script"]
                    },
                    "command": { "type": "string" },
                    "script": { "type": "string" },
                    "format": { "type": "string" },
                    "enabled": { "type": "boolean" },
                    "padding": { "type": "integer", "minimum": 0, "maximum": 64 },
                    "refreshIntervalMs": { "type": "integer", "minimum": 100, "maximum": 5000 },
                    "timeoutMs": { "type": "integer", "minimum": 100, "maximum": 30000 }
                }
            },
            "outputStyle": { "type": "string" },
            "language": { "type": "string" },
            "voiceEnabled": { "type": "boolean" },
            "editorMode": { "type": "string", "enum": ["normal", "vim"] },
            "viewMode": { "type": "string" },
            "spinnerTips": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "intervalMs": { "type": "integer", "minimum": 0 },
                    "customTips": {
                        "type": "array", "items": { "type": "string" }
                    }
                }
            },
            "terminalProgressBarEnabled": { "type": "boolean" },
            "availableModels": {
                "type": "array", "items": { "type": "string" }
            },
            "defaultModel": { "type": "string" },
            "fallbackModel": { "type": "string" },
            "fastModel": { "type": "string" },
            "sotaModel": { "type": "string" },
            "motaModel": { "type": "string" },
            "fotaModel": { "type": "string" },
            "effortLevel": { "type": "string" },
            "model_reasoning_effort": {
                "type": "string",
                "enum": ["none", "minimal", "low", "medium", "high", "xhigh"],
                "description": "Codex/OpenAI Responses reasoning effort. Directly maps to reasoning.effort for openai-codex requests."
            },
            "fastMode": { "type": "boolean" },
            "fastModePerSessionOptIn": { "type": "boolean" },
            "teammateMode": { "type": "boolean" },
            "claudeInChromeDefaultEnabled": { "type": "boolean" },
            "autoMemoryEnabled": { "type": "boolean" },
            "advisorModel": { "type": "string" },
            "systemPrompt": { "type": "string" },
            "apiKey": { "type": "string" },
            "env": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        }
    })
}
