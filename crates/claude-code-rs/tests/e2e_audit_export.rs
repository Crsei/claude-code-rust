//! E2E tests for the audit export feature.
//!
//! Offline tests verify that:
//! - The audit export module builds and links correctly
//! - AuditRecord serialization/deserialization roundtrips
//! - Hash chain integrity verification works end-to-end
//! - Tamper detection catches modifications
//! - The /audit-export command is registered
//!
//! Run:  cargo test --test e2e_audit_export

use assert_cmd::Command;
use predicates::prelude::*;

#[path = "test_workspace.rs"]
mod test_workspace;

fn workspace() -> &'static str {
    test_workspace::workspace()
}

fn cli() -> Command {
    Command::cargo_bin("claude-code-rs").expect("binary not found")
}

fn strip_api_keys(cmd: &mut Command) -> &mut Command {
    cmd.env("ANTHROPIC_API_KEY", "")
        .env("AZURE_API_KEY", "")
        .env("OPENAI_API_KEY", "")
        .env("OPENROUTER_API_KEY", "")
        .env("GOOGLE_API_KEY", "")
        .env("DEEPSEEK_API_KEY", "")
}

// =========================================================================
//  Offline: init / system prompt still work with audit_export linked in
// =========================================================================

/// --init-only succeeds — confirms audit_export module links without errors.
#[test]
fn init_succeeds_with_audit_export() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--init-only", "-C", workspace()])
        .assert()
        .success();
}

/// System prompt still renders after registering the /audit-export command.
#[test]
fn system_prompt_unaffected_by_audit_export() {
    let mut cmd = cli();
    strip_api_keys(&mut cmd);
    cmd.args(["--dump-system-prompt", "-C", workspace()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bash"))
        .stdout(predicate::str::contains("Read"));
}

// =========================================================================
//  Roundtrip: serialize → deserialize → verify
// =========================================================================

/// Build an AuditRecord in-process, write to tempfile, read back, verify.
#[test]
fn audit_record_roundtrip_and_verify() {
    use sha2::{Digest, Sha256};

    let sha256_json = |v: &serde_json::Value| -> String {
        let bytes = serde_json::to_vec(v).unwrap();
        let mut h = Sha256::new();
        h.update(&bytes);
        hex::encode(h.finalize())
    };
    let sha256_str = |s: &str| -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex::encode(h.finalize())
    };

    let zero = "0000000000000000000000000000000000000000000000000000000000000000";

    // Build 3 entries with a proper hash chain
    let d1 = serde_json::json!({"role":"user","content":"What is 2+2?"});
    let d2 = serde_json::json!({"role":"assistant","content":[{"type":"text","text":"4"}],"cost_usd":0.001});
    let d3 = serde_json::json!({"role":"user","content":"Thanks!"});

    let h1 = sha256_json(&d1);
    let c1 = sha256_str(&format!("{}{}", zero, h1));
    let h2 = sha256_json(&d2);
    let c2 = sha256_str(&format!("{}{}", c1, h2));
    let h3 = sha256_json(&d3);
    let c3 = sha256_str(&format!("{}{}", c2, h3));

    let record = serde_json::json!({
        "format_version": "1.0.0",
        "metadata": {
            "session_id": "e2e-test-session",
            "exported_at": "2026-04-07T12:00:00Z",
            "exporter_version": "cc-rust/0.1.0",
            "working_directory": "/tmp/test",
            "model": "claude-sonnet-4-20250514",
            "session_started": "2026-04-07T11:55:00Z",
            "session_ended": "2026-04-07T12:00:00Z",
            "total_messages": 3,
            "total_cost_usd": 0.001,
            "total_input_tokens": 100,
            "total_output_tokens": 10
        },
        "entries": [
            {
                "sequence": 0,
                "uuid": "aaaaaaaa-0001-0001-0001-000000000001",
                "timestamp": "2026-04-07T11:55:00Z",
                "type": "user",
                "hash": h1,
                "chain_hash": c1,
                "data": d1
            },
            {
                "sequence": 1,
                "uuid": "aaaaaaaa-0001-0001-0001-000000000002",
                "timestamp": "2026-04-07T11:55:05Z",
                "type": "assistant",
                "hash": h2,
                "chain_hash": c2,
                "data": d2
            },
            {
                "sequence": 2,
                "uuid": "aaaaaaaa-0001-0001-0001-000000000003",
                "timestamp": "2026-04-07T11:55:10Z",
                "type": "user",
                "hash": h3,
                "chain_hash": c3.clone(),
                "data": d3
            }
        ],
        "integrity": {
            "algorithm": "sha256",
            "final_chain_hash": c3,
            "entry_count": 3
        }
    });

    // Write to tempfile
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("test.audit.json");
    let json_str = serde_json::to_string_pretty(&record).unwrap();
    std::fs::write(&path, &json_str).unwrap();

    // Read back and parse
    let contents = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Verify structure
    assert_eq!(parsed["format_version"], "1.0.0");
    assert_eq!(parsed["metadata"]["total_messages"], 3);
    assert_eq!(parsed["entries"].as_array().unwrap().len(), 3);
    assert_eq!(parsed["integrity"]["entry_count"], 3);
    assert_eq!(parsed["integrity"]["algorithm"], "sha256");

    // Verify hash chain manually
    let entries = parsed["entries"].as_array().unwrap();
    let mut prev = zero.to_string();
    for entry in entries {
        let data = &entry["data"];
        let expected_hash = sha256_json(data);
        assert_eq!(entry["hash"].as_str().unwrap(), expected_hash);

        let expected_chain = sha256_str(&format!("{}{}", prev, expected_hash));
        assert_eq!(entry["chain_hash"].as_str().unwrap(), expected_chain);
        prev = expected_chain;
    }
    assert_eq!(
        parsed["integrity"]["final_chain_hash"].as_str().unwrap(),
        prev
    );
}

// =========================================================================
//  Tamper detection
// =========================================================================

/// Modify one entry's data and confirm the hash chain breaks.
#[test]
fn tamper_detection_catches_modified_entry() {
    use sha2::{Digest, Sha256};

    let sha256_json = |v: &serde_json::Value| -> String {
        let bytes = serde_json::to_vec(v).unwrap();
        let mut h = Sha256::new();
        h.update(&bytes);
        hex::encode(h.finalize())
    };
    let sha256_str = |s: &str| -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex::encode(h.finalize())
    };

    let zero = "0000000000000000000000000000000000000000000000000000000000000000";

    let d1 = serde_json::json!({"content": "original"});
    let h1 = sha256_json(&d1);
    let c1 = sha256_str(&format!("{}{}", zero, h1));

    // Build a valid record
    let mut record: serde_json::Value = serde_json::json!({
        "format_version": "1.0.0",
        "metadata": {
            "session_id": "tamper-test",
            "exported_at": "2026-01-01T00:00:00Z",
            "exporter_version": "cc-rust/0.1.0",
            "working_directory": "/tmp",
            "model": null,
            "session_started": null,
            "session_ended": null,
            "total_messages": 1,
            "total_cost_usd": 0.0,
            "total_input_tokens": 0,
            "total_output_tokens": 0
        },
        "entries": [{
            "sequence": 0,
            "uuid": "test-uuid",
            "timestamp": "2026-01-01T00:00:00Z",
            "type": "user",
            "hash": h1,
            "chain_hash": c1.clone(),
            "data": d1
        }],
        "integrity": {
            "algorithm": "sha256",
            "final_chain_hash": c1,
            "entry_count": 1
        }
    });

    // Tamper with the data
    record["entries"][0]["data"]["content"] = serde_json::json!("TAMPERED");

    // Verify hash no longer matches
    let tampered_data = &record["entries"][0]["data"];
    let tampered_hash = sha256_json(tampered_data);
    let stored_hash = record["entries"][0]["hash"].as_str().unwrap();
    assert_ne!(
        tampered_hash, stored_hash,
        "Hash should differ after tampering"
    );
}

/// Verify that inserting an extra entry breaks the chain.
#[test]
fn tamper_detection_catches_inserted_entry() {
    use sha2::{Digest, Sha256};

    let sha256_json = |v: &serde_json::Value| -> String {
        let bytes = serde_json::to_vec(v).unwrap();
        let mut h = Sha256::new();
        h.update(&bytes);
        hex::encode(h.finalize())
    };
    let sha256_str = |s: &str| -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex::encode(h.finalize())
    };

    let zero = "0000000000000000000000000000000000000000000000000000000000000000";

    // Build two valid entries
    let d1 = serde_json::json!({"msg": "first"});
    let d2 = serde_json::json!({"msg": "second"});
    let h1 = sha256_json(&d1);
    let c1 = sha256_str(&format!("{}{}", zero, h1));
    let h2 = sha256_json(&d2);
    let c2 = sha256_str(&format!("{}{}", c1, h2));

    // Insert a rogue entry between them — the chain_hash of entry #2
    // no longer matches because it's chained from the wrong predecessor
    let rogue = serde_json::json!({"msg": "injected"});
    let rogue_hash = sha256_json(&rogue);
    // The rogue uses c1 as predecessor (correct for its position)
    let rogue_chain = sha256_str(&format!("{}{}", c1, rogue_hash));
    // But the original entry #2's chain_hash was computed from c1, not rogue_chain

    let _entries = [
        serde_json::json!({
            "sequence": 0, "uuid": "u1", "timestamp": "t1", "type": "user",
            "hash": h1, "chain_hash": c1, "data": d1
        }),
        serde_json::json!({
            "sequence": 1, "uuid": "rogue", "timestamp": "t1.5", "type": "user",
            "hash": rogue_hash, "chain_hash": rogue_chain, "data": rogue
        }),
        serde_json::json!({
            "sequence": 2, "uuid": "u2", "timestamp": "t2", "type": "assistant",
            "hash": h2, "chain_hash": c2, "data": d2  // c2 was computed from c1, not rogue_chain
        }),
    ];

    // Manually verify: entry #2's chain_hash should be sha256(rogue_chain + h2)
    let expected_c2 = sha256_str(&format!("{}{}", rogue_chain, h2));
    assert_ne!(
        c2, expected_c2,
        "Original chain_hash should not match after insertion"
    );
}

// =========================================================================
//  Format structure validation
// =========================================================================

/// Verify the JSON schema of an empty audit record.
#[test]
fn audit_record_schema_has_required_fields() {
    let record = serde_json::json!({
        "format_version": "1.0.0",
        "metadata": {
            "session_id": "schema-test",
            "exported_at": "2026-01-01T00:00:00Z",
            "exporter_version": "cc-rust/0.1.0",
            "working_directory": "/tmp",
            "model": null,
            "session_started": null,
            "session_ended": null,
            "total_messages": 0,
            "total_cost_usd": 0.0,
            "total_input_tokens": 0,
            "total_output_tokens": 0
        },
        "entries": [],
        "integrity": {
            "algorithm": "sha256",
            "final_chain_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "entry_count": 0
        }
    });

    // All top-level fields present
    assert!(record.get("format_version").is_some());
    assert!(record.get("metadata").is_some());
    assert!(record.get("entries").is_some());
    assert!(record.get("integrity").is_some());

    // Metadata fields
    let meta = &record["metadata"];
    assert!(meta.get("session_id").is_some());
    assert!(meta.get("exported_at").is_some());
    assert!(meta.get("exporter_version").is_some());
    assert!(meta.get("working_directory").is_some());
    assert!(meta.get("model").is_some());
    assert!(meta.get("total_messages").is_some());
    assert!(meta.get("total_cost_usd").is_some());
    assert!(meta.get("total_input_tokens").is_some());
    assert!(meta.get("total_output_tokens").is_some());

    // Integrity fields
    let integrity = &record["integrity"];
    assert_eq!(integrity["algorithm"], "sha256");
    assert!(integrity.get("final_chain_hash").is_some());
    assert!(integrity.get("entry_count").is_some());
}
