//! Non-interactive data and agent commands. GUI startup is owned by main.
use crate::store::Store;
use serde_json::json;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const MAX_BACKUP: u64 = 20 * 1024 * 1024;

pub fn data_path() -> PathBuf {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share")
        });
    base.join("nerv-fieldnotes/inbox.sqlite3")
}

fn split_global(args: &[String]) -> Result<(Option<PathBuf>, Vec<String>), String> {
    let mut data_dir = None;
    let mut remaining = Vec::new();
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        let value = if argument == "--data-dir" {
            Some(iter.next().ok_or("--data-dir requires a directory. Example: nerv-fieldnotes --data-dir /tmp/notes")?.clone())
        } else {
            argument.strip_prefix("--data-dir=").map(str::to_owned)
        };
        if let Some(value) = value {
            if value.is_empty() || value.starts_with("--") {
                return Err("--data-dir requires a non-empty directory path".into());
            }
            if data_dir.replace(PathBuf::from(value)).is_some() {
                return Err("Pass --data-dir only once".into());
            }
        } else {
            remaining.push(argument.clone());
        }
    }
    Ok((data_dir, remaining))
}

pub fn database_path(args: &[String]) -> Result<PathBuf, String> {
    let (directory, _) = split_global(args)?;
    if let Some(directory) = directory {
        return Ok(directory.join("inbox.sqlite3"));
    }
    let has_data =
        env::var_os("XDG_DATA_HOME").is_some_and(|value| Path::new(&value).is_absolute());
    let has_home = env::var_os("HOME").is_some_and(|value| !value.is_empty());
    if !has_data && !has_home {
        return Err("Set HOME or an absolute XDG_DATA_HOME, or pass --data-dir DIR".into());
    }
    Ok(data_path())
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    let mut output = io::stdout().lock();
    serde_json::to_writer_pretty(&mut output, value)
        .map_err(|error| format!("Cannot write JSON: {error}"))?;
    writeln!(output).map_err(|error| format!("Cannot write output: {error}"))
}

fn read_backup(source: &str) -> Result<String, String> {
    let mut content = String::new();
    if source == "--stdin" {
        if io::stdin().is_terminal() {
            return Err("--stdin requires piped JSON. Example: cat backup.json | nerv-fieldnotes import --stdin".into());
        }
        io::stdin()
            .take(MAX_BACKUP + 1)
            .read_to_string(&mut content)
            .map_err(|error| format!("Cannot read backup from stdin: {error}"))?;
    } else {
        let file = fs::File::open(source)
            .map_err(|error| format!("Cannot open backup {source:?}: {error}"))?;
        file.take(MAX_BACKUP + 1)
            .read_to_string(&mut content)
            .map_err(|error| format!("Cannot read backup {source:?}: {error}"))?;
    }
    if content.len() as u64 > MAX_BACKUP {
        return Err("Backup exceeds the 20 MiB limit".into());
    }
    Ok(content)
}

fn write_export(path: &Path, content: &str, force: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".fieldnotes-export-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|error| format!("Cannot create export in {}: {error}", parent.display()))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("Cannot write export: {error}"))?;
        if force {
            fs::rename(&temporary, path)
        } else {
            fs::hard_link(&temporary, path)
        }
        .map_err(|error| {
            format!(
                "Cannot save {}: {error}. Use --force to replace an existing export.",
                path.display()
            )
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

fn help(topic: &[String]) -> &'static str {
    match topic.first().map(String::as_str) {
        Some("inbox") => match topic.get(1).map(String::as_str) {
            Some("list") => {
                "Usage: nerv-fieldnotes [--data-dir DIR] inbox list [--agent NAME] [--all]\n\nList pending handoffs as JSON. --all includes acknowledged items.\nExamples:\n  fieldnotes-agent list --agent codex\n  nerv-fieldnotes inbox list --all"
            }
            Some("show") => {
                "Usage: nerv-fieldnotes [--data-dir DIR] inbox show HANDOFF_ID\n\nRead one complete handoff as JSON.\nExample:\n  fieldnotes-agent show HANDOFF_ID"
            }
            Some("send") => {
                "Usage: nerv-fieldnotes [--data-dir DIR] inbox send --stdin\n\nQueue one handoff JSON payload from stdin. Preserve its ID when retrying.\nExample:\n  cat handoff.json | fieldnotes-agent send --stdin"
            }
            Some("ack") => {
                "Usage: nerv-fieldnotes [--data-dir DIR] inbox ack HANDOFF_ID\n\nAcknowledge receipt. Repeated acknowledgement is safe.\nExample:\n  fieldnotes-agent ack HANDOFF_ID"
            }
            _ => {
                "Usage: nerv-fieldnotes [--data-dir DIR] inbox <list|show|ack|send>\n\nRead and acknowledge the durable local agent inbox.\nExamples:\n  fieldnotes-agent list --agent codex\n  fieldnotes-agent show HANDOFF_ID\n  fieldnotes-agent ack HANDOFF_ID\n\nRun a subcommand with --help for its options."
            }
        },
        Some("import") => {
            "Usage: nerv-fieldnotes [--data-dir DIR] import FILE|--stdin\n\nValidate and merge a version 1 JSON backup. Newer saved notes win.\nExamples:\n  nerv-fieldnotes import fieldnotes-backup.json\n  cat backup.json | nerv-fieldnotes import --stdin"
        }
        Some("export") => {
            "Usage: nerv-fieldnotes [--data-dir DIR] export FILE|--stdout [--force]\n\nExport every note, including archived and trashed notes.\n--force replaces an existing file.\nExamples:\n  nerv-fieldnotes export fieldnotes-backup.json\n  nerv-fieldnotes export --stdout > backup.json"
        }
        _ => {
            "NERV Fieldnotes 2.0.0\n\nUsage: nerv-fieldnotes [--data-dir DIR] [COMMAND]\n\nWithout a command, open the most recently edited active sticky note.\nCommands:\n  inbox list|show|ack|send  Read, acknowledge, or enqueue handoffs\n  import FILE|--stdin  Merge a version 1 JSON backup\n  export FILE|--stdout Export all saved notes\n\nOptions:\n  --board              Open the complete note board\n  --new-note           Open a new sticky note\n  --data-dir DIR       Store native notes and inbox in this directory\n  --help              Show command help\n  --version           Print the application version\n\nExamples:\n  nerv-fieldnotes\n  fieldnotes-agent list --agent codex\n  nerv-fieldnotes import backup.json\n  nerv-fieldnotes export --stdout > backup.json\n\nRun a command with --help for details."
        }
    }
}

pub fn run(args: &[String]) -> Result<bool, String> {
    let original_args = args;
    let (_, args) = split_global(args)?;
    if args.is_empty()
        || args
            .first()
            .is_some_and(|arg| matches!(arg.as_str(), "--smoke-test" | "--board" | "--new-note"))
    {
        return Ok(false);
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", help(&args));
        return Ok(true);
    }
    if args == ["--version"] {
        println!("nerv-fieldnotes {}", env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }
    let database = database_path(original_args)?;
    match args[0].as_str() {
        "inbox" => run_inbox(&args[1..], &database)?,
        "import" => {
            if args.len() != 2 || (args[1].starts_with('-') && args[1] != "--stdin") {
                return Err(help(&args).into());
            }
            let content = read_backup(&args[1])?;
            let store = Store::open(&database)?;
            let count = store.import_backup(&content)?;
            print_json(&json!({"ok": true, "imported": count}))?;
        }
        "export" => {
            let force = args.iter().skip(2).any(|arg| arg == "--force");
            if args.len() < 2
                || args.len() > 3
                || (args.len() == 3 && args[2] != "--force")
                || (args[1].starts_with('-') && args[1] != "--stdout")
            {
                return Err(help(&args).into());
            }
            let store = Store::open(&database)?;
            let backup = store.export_backup()?;
            if args[1] == "--stdout" {
                println!("{backup}");
            } else {
                write_export(Path::new(&args[1]), &backup, force)?;
                print_json(&json!({"ok": true, "path": args[1]}))?;
            }
        }
        _ => {
            return Err(format!(
                "Unknown command {:?}. Run nerv-fieldnotes --help for examples.",
                args[0]
            ));
        }
    }
    Ok(true)
}

fn run_inbox(args: &[String], database: &Path) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(help(&["inbox".into()]).into());
    };
    match command {
        "list" => {
            let mut agent = None;
            let mut all = false;
            let mut iter = args[1..].iter();
            while let Some(argument) = iter.next() {
                match argument.as_str() {
                    "--all" => all = true,
                    "--agent" => {
                        let value = iter.next().ok_or(
                            "--agent requires a name. Example: fieldnotes-agent list --agent codex",
                        )?;
                        if value.is_empty()
                            || value.starts_with("--")
                            || agent.replace(value).is_some()
                        {
                            return Err("Pass one non-empty --agent name".into());
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Unknown list option {argument:?}. Run fieldnotes-agent list --help."
                        ));
                    }
                }
            }
            let store = Store::open(database)?;
            let items = store
                .inbox()?
                .into_iter()
                .filter(|item| {
                    (all || item.status == "pending")
                        && agent.is_none_or(|name| item.agent == *name)
                })
                .collect::<Vec<_>>();
            print_json(
                &json!({"items": items, "inboxPath": database.parent().unwrap_or(Path::new(".")).join("inbox")}),
            )?;
        }
        "send" => {
            if args.len() != 2 || args[1] != "--stdin" {
                return Err(help(&["inbox".into(), "send".into()]).into());
            }
            let content = read_backup("--stdin")?;
            if content.len() > 128 * 1024 {
                return Err("Handoff exceeds the 128 KiB limit".into());
            }
            let store = Store::open(database)?;
            let item = store.receive_handoff(&content)?;
            print_json(&json!({"ok": true, "item": item}))?;
        }
        "show" | "ack" => {
            if args.len() != 2 || args[1].starts_with('-') {
                return Err(help(&["inbox".into(), command.into()]).into());
            }
            let store = Store::open(database)?;
            if command == "ack" {
                store.acknowledge(&args[1])?;
            }
            let item = store
                .inbox()?
                .into_iter()
                .find(|item| item.id == args[1])
                .ok_or("Handoff not found. Run fieldnotes-agent list --all.")?;
            if command == "ack" {
                print_json(&json!({"ok": true, "item": item}))?;
            } else {
                print_json(&item)?;
            }
        }
        _ => {
            return Err(format!(
                "Unknown inbox command {command:?}. Run fieldnotes-agent --help."
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn data_dir_is_extracted_before_or_after_commands() {
        let before = args(&["--data-dir", "/tmp/one", "inbox", "list"]);
        let after = args(&["inbox", "list", "--data-dir=/tmp/one"]);
        assert_eq!(
            database_path(&before).unwrap(),
            PathBuf::from("/tmp/one/inbox.sqlite3")
        );
        assert_eq!(
            split_global(&before).unwrap(),
            split_global(&after).unwrap()
        );
    }

    #[test]
    fn invalid_data_dir_options_fail_before_storage_is_opened() {
        for values in [
            &["--data-dir"][..],
            &["--data-dir=..", "--data-dir", "/tmp/a"],
            &["--data-dir", "--help"],
            &["--data-dir="],
        ] {
            assert!(database_path(&args(values)).is_err());
        }
    }

    #[test]
    fn export_refuses_existing_files_and_keeps_original_bytes() {
        let temporary = tempfile::tempdir().unwrap();
        let target = temporary.path().join("backup.json");
        write_export(&target, "first", false).unwrap();
        assert!(write_export(&target, "second", false).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "first");
        write_export(&target, "second", true).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "second");
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 1);
    }

    #[test]
    fn invalid_cli_commands_are_actionable_and_noninteractive() {
        assert!(run(&args(&["typo"])).unwrap_err().contains("--help"));
        assert!(run(&args(&["import"])).unwrap_err().contains("import FILE"));
        assert!(run(&args(&["export"])).unwrap_err().contains("export FILE"));
        assert!(!run(&[]).unwrap());
        assert!(!run(&args(&["--smoke-test"])).unwrap());
        assert!(!run(&args(&["--board"])).unwrap());
        assert!(!run(&args(&["--new-note"])).unwrap());
    }

    #[test]
    fn help_and_version_do_not_create_a_database() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("unused");
        let flag = format!("--data-dir={}", path.display());
        assert!(run(&[flag.clone(), "--help".into()]).unwrap());
        assert!(run(&[flag, "--version".into()]).unwrap());
        assert!(!path.exists());
    }
    #[test]
    fn cli_import_export_round_trip_uses_the_requested_database() {
        let temporary = tempfile::tempdir().unwrap();
        let source_dir = temporary.path().join("source");
        let source = Store::open(source_dir.join("inbox.sqlite3")).unwrap();
        let mut note = crate::model::Note::new();
        note.title = "CLI round trip".into();
        source.save_note(&note).unwrap();
        let backup = temporary.path().join("backup.json");
        assert!(
            run(&[
                "--data-dir".into(),
                source_dir.display().to_string(),
                "export".into(),
                backup.display().to_string(),
            ])
            .unwrap()
        );
        let destination = temporary.path().join("destination");
        assert!(
            run(&[
                "import".into(),
                backup.display().to_string(),
                "--data-dir".into(),
                destination.display().to_string(),
            ])
            .unwrap()
        );
        let imported = Store::open(destination.join("inbox.sqlite3"))
            .unwrap()
            .load_notes()
            .unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].id, note.id);
        assert_eq!(imported[0].title, "CLI round trip");
    }

    #[test]
    fn cli_acknowledgement_is_durable_and_repeatable() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("inbox.sqlite3");
        let store = Store::open(&database).unwrap();
        let mut note = crate::model::Note::new();
        note.title = "CLI inbox receipt".into();
        let item = store.handoff(&note, "codex", "Review the change").unwrap();
        drop(store);
        let command = vec![
            "inbox".into(),
            "ack".into(),
            item.id.clone(),
            "--data-dir".into(),
            temporary.path().display().to_string(),
        ];
        assert!(run(&command).unwrap());
        assert!(run(&command).unwrap());
        let received = Store::open(database).unwrap().inbox().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].status, "acknowledged");
    }
}
