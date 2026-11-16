use crate::model::{Backup, Handoff, InboxItem, Note, timestamp};
use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

pub struct Store {
    connection: Connection,
    path: PathBuf,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Cannot create the notes directory: {error}"))?;
        }
        let connection = Connection::open(&path)
            .map_err(|error| format!("Cannot open notes database: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(db_error)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS native_notes (
               id TEXT PRIMARY KEY, payload TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS native_drafts (
               id TEXT PRIMARY KEY, payload TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS native_settings (
               key TEXT PRIMARY KEY, value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS inbox (
               id TEXT PRIMARY KEY, payload TEXT NOT NULL,
               status TEXT NOT NULL, received_at TEXT NOT NULL
             );",
            )
            .map_err(db_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("Cannot protect notes database permissions: {error}"))?;
        }
        Ok(Self { connection, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_notes(&self) -> Result<Vec<Note>, String> {
        read_notes(&self.connection, false)
    }

    pub fn save_note(&self, note: &Note) -> Result<(), String> {
        self.write_note(note, false)
    }

    pub fn commit_note(&self, note: &Note) -> Result<(), String> {
        self.write_note(note, true)
    }

    fn write_note(&self, note: &Note, clear_draft: bool) -> Result<(), String> {
        note.validate()?;
        let payload = serde_json::to_string(note).map_err(json_error)?;
        let tx = self.connection.unchecked_transaction().map_err(db_error)?;
        tx.execute(
            "INSERT INTO native_notes (id, payload) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
            params![note.id, payload],
        )
        .map_err(db_error)?;
        if clear_draft {
            tx.execute("DELETE FROM native_drafts WHERE id=?1", [&note.id])
                .map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    }

    pub fn delete_note(&self, id: &str) -> Result<(), String> {
        let tx = self.connection.unchecked_transaction().map_err(db_error)?;
        tx.execute("DELETE FROM native_notes WHERE id=?1", [id])
            .map_err(db_error)?;
        tx.execute("DELETE FROM native_drafts WHERE id=?1", [id])
            .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }

    pub fn save_draft(&self, note: &Note) -> Result<(), String> {
        note.validate()?;
        let payload = serde_json::to_string(note).map_err(json_error)?;
        self.connection
            .execute(
                "INSERT INTO native_drafts (id,payload) VALUES (?1,?2)
             ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
                params![note.id, payload],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn load_draft(&self, id: &str) -> Result<Option<Note>, String> {
        let payload: Option<String> = self
            .connection
            .query_row(
                "SELECT payload FROM native_drafts WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        payload.map(|raw| parse_stored_note(&raw)).transpose()
    }

    pub fn load_drafts(&self) -> Result<Vec<Note>, String> {
        read_notes(&self.connection, true)
    }

    pub fn delete_draft(&self, id: &str) -> Result<(), String> {
        self.connection
            .execute("DELETE FROM native_drafts WHERE id=?1", [id])
            .map_err(db_error)?;
        Ok(())
    }

    pub fn import_backup(&self, raw: &str) -> Result<usize, String> {
        let backup = Backup::parse(raw)?;
        let tx = self.connection.unchecked_transaction().map_err(db_error)?;
        let existing: HashMap<_, _> = read_notes(&tx, false)?
            .into_iter()
            .map(|note| (note.id.clone(), note))
            .collect();
        let mut changed = 0;
        for note in backup.notes {
            let newer = match existing.get(&note.id) {
                None => true,
                Some(local) => {
                    DateTime::parse_from_rfc3339(&note.updated_at)
                        .map_err(|error| error.to_string())?
                        > DateTime::parse_from_rfc3339(&local.updated_at)
                            .map_err(|error| error.to_string())?
                }
            };
            if newer {
                tx.execute(
                    "INSERT INTO native_notes (id,payload) VALUES (?1,?2)
                     ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
                    params![note.id, serde_json::to_string(&note).map_err(json_error)?],
                )
                .map_err(db_error)?;
                changed += 1;
            }
        }
        tx.commit().map_err(db_error)?;
        Ok(changed)
    }

    pub fn export_backup(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&Backup {
            version: 1,
            notes: self.load_notes()?,
        })
        .map_err(json_error)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        self.connection
            .query_row(
                "SELECT value FROM native_settings WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        if key.len() > 100 || value.len() > 100_000 {
            return Err("Preference key or value exceeds its size limit.".into());
        }
        self.connection
            .execute(
                "INSERT INTO native_settings (key,value) VALUES (?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn handoff(
        &self,
        note: &Note,
        agent: &str,
        instructions: &str,
    ) -> Result<InboxItem, String> {
        note.validate()?;
        let handoff = Handoff {
            id: Uuid::new_v4().to_string(),
            note_id: note.id.clone(),
            title: if note.title.is_empty() {
                "Untitled note".into()
            } else {
                note.title.clone()
            },
            body: note.markdown(agent, instructions),
            agent: if agent.trim().is_empty() {
                "Agent".into()
            } else {
                agent.trim().to_string()
            },
            created_at: timestamp(),
        };
        self.receive_handoff(&serde_json::to_string(&handoff).map_err(json_error)?)
    }

    pub fn receive_handoff(&self, raw: &str) -> Result<InboxItem, String> {
        if raw.len() > 128 * 1024 {
            return Err("Handoff exceeds the 128 KiB request limit.".into());
        }
        let handoff: Handoff = serde_json::from_str(raw).map_err(json_error)?;
        validate_handoff(&handoff)?;
        let tx = self.connection.unchecked_transaction().map_err(db_error)?;
        let existing: Option<(String, String, String)> = tx
            .query_row(
                "SELECT payload,status,received_at FROM inbox WHERE id=?1",
                [&handoff.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((payload, status, received_at)) = existing {
            let previous: Handoff = serde_json::from_str(&payload).map_err(json_error)?;
            if previous != handoff {
                return Err("This handoff ID already exists with different content.".into());
            }
            return Ok(InboxItem::from_handoff(handoff, status, received_at));
        }
        let received_at = timestamp();
        // Match Python's sorted, Unicode-preserving JSON encoding so either runtime can retry a delivery.
        let payload = compatible_payload(&handoff)?;
        let directory = self
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("inbox");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Cannot create agent inbox directory: {error}"))?;
        let destination = directory.join(format!("{}.md", handoff.id));
        let temporary = directory.join(format!("{}.{}.native-tmp", handoff.id, Uuid::new_v4()));
        let document = format!(
            "# {}\n\nAgent: {}\nHandoff ID: {}\nNote ID: {}\nCreated: {}\n\n{}\n",
            handoff.title,
            handoff.agent,
            handoff.id,
            handoff.note_id,
            handoff.created_at,
            handoff.body
        );
        write_private(&temporary, document.as_bytes())?;
        fs::rename(&temporary, &destination)
            .map_err(|error| format!("Cannot save Markdown handoff: {error}"))?;
        tx.execute(
            "INSERT INTO inbox(id,payload,status,received_at) VALUES (?1,?2,'pending',?3)",
            params![handoff.id, payload, received_at],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(InboxItem::from_handoff(
            handoff,
            "pending".into(),
            received_at,
        ))
    }

    pub fn inbox(&self) -> Result<Vec<InboxItem>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT payload,status,received_at FROM inbox ORDER BY received_at DESC,id DESC",
            )
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(db_error)?;
        rows.map(|row| {
            let (payload, status, received_at) = row.map_err(db_error)?;
            let handoff: Handoff = serde_json::from_str(&payload).map_err(json_error)?;
            validate_handoff(&handoff)?;
            if status != "pending" && status != "acknowledged" {
                return Err("The inbox contains an unknown delivery state.".into());
            }
            Ok(InboxItem::from_handoff(handoff, status, received_at))
        })
        .collect()
    }

    pub fn acknowledge(&self, id: &str) -> Result<(), String> {
        let changed = self
            .connection
            .execute("UPDATE inbox SET status='acknowledged' WHERE id=?1", [id])
            .map_err(db_error)?;
        if changed == 0 {
            return Err("Handoff not found.".into());
        }
        Ok(())
    }
}

fn read_notes(connection: &Connection, drafts: bool) -> Result<Vec<Note>, String> {
    let query = if drafts {
        "SELECT payload FROM native_drafts"
    } else {
        "SELECT payload FROM native_notes"
    };
    let mut statement = connection.prepare(query).map_err(db_error)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut notes = rows
        .map(|raw| parse_stored_note(&raw.map_err(db_error)?))
        .collect::<Result<Vec<_>, _>>()?;
    notes.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(notes)
}

fn parse_stored_note(raw: &str) -> Result<Note, String> {
    let note: Note = serde_json::from_str(raw).map_err(|error| {
        format!("A saved note is unreadable. The original data was preserved: {error}")
    })?;
    note.validate()?;
    Ok(note)
}

fn validate_handoff(handoff: &Handoff) -> Result<(), String> {
    for id in [&handoff.id, &handoff.note_id] {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(
                "Handoff IDs must be 1-128 ASCII letters, digits, underscores, or hyphens.".into(),
            );
        }
    }
    if handoff.title.chars().count() > 500
        || handoff.body.chars().count() > 100_000
        || handoff.agent.trim().is_empty()
        || handoff.agent.chars().count() > 100
    {
        return Err(
            "Handoff title, body, or agent exceeds the allowed limits, or agent is empty.".into(),
        );
    }
    DateTime::parse_from_rfc3339(&handoff.created_at)
        .map_err(|_| "The handoff timestamp must be ISO 8601 with a timezone.".to_string())?;
    Ok(())
}

fn compatible_payload(handoff: &Handoff) -> Result<String, String> {
    let value = serde_json::to_value(handoff).map_err(json_error)?;
    let object = value
        .as_object()
        .ok_or("Cannot serialize handoff fields.")?;
    let mut fields = object.iter().collect::<Vec<_>>();
    fields.sort_by_key(|(key, _)| *key);
    Ok(format!(
        "{{{}}}",
        fields
            .into_iter()
            .map(|(key, value)| {
                Ok(format!(
                    "{}: {}",
                    serde_json::to_string(key).map_err(json_error)?,
                    serde_json::to_string(value).map_err(json_error)?
                ))
            })
            .collect::<Result<Vec<_>, String>>()?
            .join(", ")
    ))
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Cannot write Markdown handoff: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("Cannot write Markdown handoff: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("Cannot flush Markdown handoff: {error}"))
}

fn db_error(error: rusqlite::Error) -> String {
    format!("Notes database error: {error}")
}

fn json_error(error: serde_json::Error) -> String {
    format!("Invalid Fieldnotes JSON: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, NoteStatus};
    use tempfile::tempdir;

    #[test]
    fn saved_notes_and_settings_survive_reopening_the_actual_database() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inbox.sqlite3");
        let mut note = Note::new();
        note.title = "Native memory".into();
        note.body = "Unicode 日本語".into();
        note.color = Color::Amber;
        note.status = NoteStatus::Archived;
        {
            let store = Store::open(&path).unwrap();
            store.save_note(&note).unwrap();
            store.set_setting("theme", "dark").unwrap();
        }
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.load_notes().unwrap(), vec![note]);
        assert_eq!(
            reopened.get_setting("theme").unwrap().as_deref(),
            Some("dark")
        );
    }

    #[test]
    fn independent_drafts_survive_and_committing_clears_only_the_matching_draft() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notes.db");
        let a = Note::new();
        let b = Note::new();
        {
            let store = Store::open(&path).unwrap();
            store.save_draft(&a).unwrap();
            store.save_draft(&b).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.load_drafts().unwrap().len(), 2);
        assert_eq!(store.load_draft(&a.id).unwrap(), Some(a.clone()));
        store.commit_note(&a).unwrap();
        assert!(store.load_draft(&a.id).unwrap().is_none());
        assert!(store.load_draft(&b.id).unwrap().is_some());
    }

    #[test]
    fn saving_card_metadata_preserves_an_existing_recovery_draft() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("notes.db")).unwrap();
        let mut note = Note::new();
        note.title = "Saved title".into();
        store.save_note(&note).unwrap();
        let mut draft = note.clone();
        draft.title = "Unsaved edit".into();
        store.save_draft(&draft).unwrap();
        note.pinned = true;
        store.save_note(&note).unwrap();
        assert_eq!(store.load_draft(&note.id).unwrap(), Some(draft));
        assert!(store.load_notes().unwrap()[0].pinned);
    }

    #[test]
    fn deleting_a_note_removes_its_draft_but_keeps_unrelated_notes() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("notes.db")).unwrap();
        let a = Note::new();
        let b = Note::new();
        store.save_note(&a).unwrap();
        store.save_note(&b).unwrap();
        store.save_draft(&a).unwrap();
        store.delete_note(&a.id).unwrap();
        assert_eq!(store.load_notes().unwrap(), vec![b]);
        assert!(store.load_draft(&a.id).unwrap().is_none());
    }

    #[test]
    fn invalid_import_is_atomic_and_cannot_replace_existing_notes() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("notes.db")).unwrap();
        let mut original = Note::new();
        original.title = "Keep me".into();
        store.save_note(&original).unwrap();
        let mut invalid = Note::new();
        invalid.updated_at = "invalid".into();
        let raw = serde_json::to_string(&Backup {
            version: 1,
            notes: vec![Note::new(), invalid],
        })
        .unwrap();
        assert!(store.import_backup(&raw).is_err());
        assert_eq!(store.load_notes().unwrap(), vec![original]);
    }

    #[test]
    fn importing_newer_backup_merges_ids_and_preserves_newer_local_edits() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("notes.db")).unwrap();
        let mut note = Note::new();
        note.updated_at = "2026-01-02T00:00:00Z".into();
        note.title = "Local".into();
        store.save_note(&note).unwrap();
        let mut old = note.clone();
        old.title = "Old import".into();
        old.updated_at = "2026-01-01T00:00:00Z".into();
        let raw = serde_json::to_string(&Backup {
            version: 1,
            notes: vec![old],
        })
        .unwrap();
        assert_eq!(store.import_backup(&raw).unwrap(), 0);
        assert_eq!(store.load_notes().unwrap()[0].title, "Local");
        let mut newer = note.clone();
        newer.title = "New import".into();
        newer.updated_at = "2026-01-03T00:00:00Z".into();
        let raw = serde_json::to_string(&Backup {
            version: 1,
            notes: vec![newer.clone(), Note::new()],
        })
        .unwrap();
        assert_eq!(store.import_backup(&raw).unwrap(), 2);
        assert_eq!(store.load_notes().unwrap().len(), 2);
        assert!(
            store
                .load_notes()
                .unwrap()
                .iter()
                .any(|saved| saved == &newer)
        );
        let exported = Backup::parse(&store.export_backup().unwrap()).unwrap();
        assert_eq!(exported.notes.len(), 2);
    }

    #[test]
    fn native_handoff_is_durable_and_compatible_with_existing_python_schema() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inbox.sqlite3");
        // The Python runtime may already have initialized this exact table.
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch("CREATE TABLE inbox (id TEXT PRIMARY KEY,payload TEXT NOT NULL,status TEXT NOT NULL,received_at TEXT NOT NULL)").unwrap();
        }
        let mut note = Note::new();
        note.title = "Review the native app".into();
        note.body = "No webview.".into();
        let id;
        {
            let store = Store::open(&path).unwrap();
            let item = store.handoff(&note, "codex", "Verify persistence").unwrap();
            id = item.id;
            assert_eq!(item.status, "pending");
            let markdown =
                fs::read_to_string(dir.path().join("inbox").join(format!("{id}.md"))).unwrap();
            assert!(markdown.contains("Verify persistence"));
        }
        let store = Store::open(&path).unwrap();
        let items = store.inbox().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, id);
        store.acknowledge(&id).unwrap();
        store.acknowledge(&id).unwrap();
        assert_eq!(store.inbox().unwrap()[0].status, "acknowledged");
        assert!(store.acknowledge("missing").is_err());
    }

    #[test]
    fn stdin_handoff_delivery_is_idempotent_and_conflicting_ids_are_rejected() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("notes.db")).unwrap();
        let handoff = Handoff {
            id: "retry-me".into(),
            note_id: "source-note".into(),
            title: "Task".into(),
            body: "Keep intact".into(),
            agent: "codex".into(),
            created_at: "2026-11-16T08:00:00Z".into(),
        };
        let raw = serde_json::to_string(&handoff).unwrap();
        let first = store.receive_handoff(&raw).unwrap();
        let second = store.receive_handoff(&raw).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(store.inbox().unwrap().len(), 1);
        store.acknowledge(&first.id).unwrap();
        assert_eq!(store.receive_handoff(&raw).unwrap().status, "acknowledged");
        let changed = raw.replace("Keep intact", "Changed");
        assert!(store.receive_handoff(&changed).is_err());
    }

    #[test]
    fn a_temporary_export_left_by_a_crash_does_not_block_a_retry() {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("notes.db")).unwrap();
        fs::create_dir(dir.path().join("inbox")).unwrap();
        fs::write(dir.path().join("inbox/retry.native-tmp"), "interrupted").unwrap();
        let handoff = Handoff {
            id: "retry".into(),
            note_id: "source-note".into(),
            title: "Task".into(),
            body: "Preserve the handoff".into(),
            agent: "codex".into(),
            created_at: "2026-11-16T08:00:00Z".into(),
        };
        let raw = serde_json::to_string(&handoff).unwrap();
        assert!(store.receive_handoff(&raw).is_ok());
        assert_eq!(store.inbox().unwrap().len(), 1);
    }

    #[test]
    fn unreadable_database_is_preserved_and_reported() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("broken.db");
        fs::write(&path, b"this is not a sqlite database").unwrap();
        assert!(Store::open(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"this is not a sqlite database");
    }

    #[test]
    fn invalid_stored_note_is_reported_without_replacing_original_payload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notes.db");
        let store = Store::open(&path).unwrap();
        store
            .connection
            .execute("INSERT INTO native_notes VALUES ('broken','not json')", [])
            .unwrap();
        assert!(store.load_notes().is_err());
        let raw: String = store
            .connection
            .query_row(
                "SELECT payload FROM native_notes WHERE id='broken'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, "not json");
    }
}
