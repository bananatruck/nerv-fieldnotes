//! Actual executable tests. These run headlessly without a desktop or server.
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn invoke(directory: &Path, arguments: &[&str], input: Option<&str>) -> Output {
    let binary = std::env::var_os("NERV_FIELDNOTES_BIN")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_nerv-fieldnotes").into());
    let mut child = Command::new(binary)
        .arg("--data-dir")
        .arg(directory)
        .args(arguments)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Launch native CLI binary");
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().expect("Wait for native CLI")
}

fn successful(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI success output is JSON")
}

fn note() -> Value {
    json!({
        "id": "cli-process-note", "title": "Actual native process", "body": "Persist this thought.",
        "tags": ["native", "test"], "color": "amber", "pinned": true,
        "status": "active", "checklist": false,
        "createdAt": "2026-11-16T08:00:00Z", "updatedAt": "2026-11-16T08:00:00Z"
    })
}

#[test]
fn headless_help_and_version_do_not_create_data() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("unused");
    for arguments in [
        vec!["--version"],
        vec!["--help"],
        vec!["inbox", "send", "--help"],
    ] {
        let output = invoke(&directory, &arguments, None);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
    }
    assert!(!directory.exists());
}

#[test]
fn actual_process_import_export_is_durable_and_invalid_backups_do_not_replace_notes() {
    let temporary = tempfile::tempdir().unwrap();
    let data = temporary.path().join("data");
    let backup = json!({"version": 1, "notes": [note()]}).to_string();
    let imported = successful(invoke(&data, &["import", "--stdin"], Some(&backup)));
    assert_eq!(imported["imported"], 1);
    let exported = successful(invoke(&data, &["export", "--stdout"], None));
    assert_eq!(exported["notes"][0]["title"], "Actual native process");
    let malformed = invoke(
        &data,
        &["import", "--stdin"],
        Some("{\"version\":1,\"notes\":[{}]}"),
    );
    assert!(!malformed.status.success());
    let unchanged = successful(invoke(&data, &["export", "--stdout"], None));
    assert_eq!(unchanged["notes"], exported["notes"]);
    let path = temporary.path().join("backup with spaces.json");
    successful(invoke(&data, &["export", path.to_str().unwrap()], None));
    assert_eq!(
        serde_json::from_str::<Value>(&fs::read_to_string(&path).unwrap()).unwrap()["notes"],
        exported["notes"]
    );
    let original = fs::read(&path).unwrap();
    assert!(
        !invoke(&data, &["export", path.to_str().unwrap()], None)
            .status
            .success()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    successful(invoke(
        &data,
        &["export", path.to_str().unwrap(), "--force"],
        None,
    ));
}

#[test]
fn actual_process_inbox_send_list_show_and_ack_preserve_idempotency() {
    let temporary = tempfile::tempdir().unwrap();
    let data = temporary.path().join("data");
    let sentinel = temporary.path().join("must-not-execute");
    let mut payload = json!({
        "id": "cli-process-handoff", "noteId": "cli-process-note", "title": "Agent delivery",
        "body": format!("touch {}", sentinel.display()), "agent": "codex",
        "createdAt": "2026-11-16T08:00:00Z"
    });
    let serialized = payload.to_string();
    successful(invoke(
        &data,
        &["inbox", "send", "--stdin"],
        Some(&serialized),
    ));
    successful(invoke(
        &data,
        &["inbox", "send", "--stdin"],
        Some(&serialized),
    ));
    let listed = successful(invoke(&data, &["inbox", "list", "--agent", "codex"], None));
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed["items"][0]["status"], "pending");
    let other_agent = successful(invoke(&data, &["inbox", "list", "--agent", "other"], None));
    assert!(other_agent["items"].as_array().unwrap().is_empty());
    let shown = successful(invoke(
        &data,
        &["inbox", "show", "cli-process-handoff"],
        None,
    ));
    assert_eq!(shown["body"], payload["body"]);
    assert!(!sentinel.exists());
    let markdown = fs::read_to_string(data.join("inbox/cli-process-handoff.md")).unwrap();
    assert!(markdown.contains("Agent delivery"));
    for _ in 0..2 {
        let acknowledged = successful(invoke(
            &data,
            &["inbox", "ack", "cli-process-handoff"],
            None,
        ));
        assert_eq!(acknowledged["item"]["status"], "acknowledged");
    }
    assert!(
        successful(invoke(&data, &["inbox", "list"], None))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let all = successful(invoke(&data, &["inbox", "list", "--all"], None));
    assert_eq!(all["items"].as_array().unwrap().len(), 1);
    payload["title"] = json!("Conflicting replacement");
    assert!(
        !invoke(
            &data,
            &["inbox", "send", "--stdin"],
            Some(&payload.to_string())
        )
        .status
        .success()
    );
    assert_eq!(
        successful(invoke(
            &data,
            &["inbox", "show", "cli-process-handoff"],
            None
        ))["title"],
        "Agent delivery"
    );
}

#[test]
fn missing_arguments_and_unknown_ids_fail_without_interaction() {
    let temporary = tempfile::tempdir().unwrap();
    for arguments in [
        vec!["import"],
        vec!["export"],
        vec!["inbox", "show"],
        vec!["inbox", "ack", "missing"],
        vec!["inbox", "list", "--agent"],
    ] {
        let output = invoke(temporary.path(), &arguments, None);
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}
