use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    #[default]
    Paper,
    Amber,
    Red,
    Green,
    Purple,
}

impl Color {
    pub const ALL: [Self; 5] = [
        Self::Paper,
        Self::Amber,
        Self::Red,
        Self::Green,
        Self::Purple,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Amber => "amber",
            Self::Red => "red",
            Self::Green => "green",
            Self::Purple => "purple",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Paper => "Paper",
            Self::Amber => "Amber",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Purple => "Purple",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteStatus {
    #[default]
    Active,
    Archived,
    Deleted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    All,
    Pinned,
    Archive,
    Trash,
    Inbox,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub color: Color,
    pub pinned: bool,
    pub status: NoteStatus,
    pub created_at: String,
    pub updated_at: String,
    pub checklist: bool,
}

impl Default for Note {
    fn default() -> Self {
        Self::new()
    }
}

impl Note {
    pub fn new() -> Self {
        let now = timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            title: String::new(),
            body: String::new(),
            tags: Vec::new(),
            color: Color::Paper,
            pinned: false,
            status: NoteStatus::Active,
            created_at: now.clone(),
            updated_at: now,
            checklist: false,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 100
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(
                "Note IDs must be 1-100 ASCII letters, digits, underscores, or hyphens.".into(),
            );
        }
        if self.title.chars().count() > 500 || self.body.chars().count() > 100_000 {
            return Err(
                "A note may contain at most 500 title characters and 100,000 body characters."
                    .into(),
            );
        }
        if self.tags.len() > 20 || self.tags.iter().any(|tag| tag.chars().count() > 40) {
            return Err("A note may contain at most 20 tags of 40 characters each.".into());
        }
        DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|_| "The note creation timestamp is invalid.".to_string())?;
        DateTime::parse_from_rfc3339(&self.updated_at)
            .map_err(|_| "The note update timestamp is invalid.".to_string())?;
        Ok(())
    }

    pub fn markdown(&self, agent: &str, instructions: &str) -> String {
        let title = if self.title.is_empty() {
            "Untitled note"
        } else {
            &self.title
        };
        let agent = if agent.trim().is_empty() {
            "Agent"
        } else {
            agent.trim()
        };
        let tags = if self.tags.is_empty() {
            "none".into()
        } else {
            self.tags.join(", ")
        };
        let mut text = format!(
            "# {title}\n\n{}\n\n---\nAssigned to: {agent}\nTags: {tags}\nNote ID: {}\nUpdated: {}\n",
            self.body, self.id, self.updated_at
        );
        if !instructions.trim().is_empty() {
            text.push_str(&format!("\nInstructions:\n{}\n", instructions.trim()));
        }
        text
    }

    pub fn toggle_task(&mut self, index: usize) {
        let lines: Vec<String> = self
            .body
            .split('\n')
            .enumerate()
            .map(|(line_index, line)| {
                if line_index != index {
                    return line.to_string();
                }
                let trimmed = line.trim_start();
                let indent = &line[..line.len() - trimmed.len()];
                if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
                    format!("{indent}- [ ]{}", &trimmed[5..])
                } else if let Some(rest) = trimmed.strip_prefix("- [ ]") {
                    format!("{indent}- [x]{rest}")
                } else {
                    format!("- [x] {line}")
                }
            })
            .collect();
        self.body = lines.join("\n");
        self.touch();
    }
}

pub fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn parse_tags(raw: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    raw.split(',')
        .map(|tag| {
            tag.trim()
                .strip_prefix('#')
                .unwrap_or(tag.trim())
                .chars()
                .take(40)
                .collect::<String>()
        })
        .filter(|tag| !tag.is_empty() && seen.insert(tag.clone()))
        .take(20)
        .collect()
}

pub fn filter_notes(notes: &[Note], view: View, query: &str, tag: &str) -> Vec<Note> {
    let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut matches: Vec<_> = notes
        .iter()
        .filter(|note| {
            let in_view = match view {
                View::All => note.status == NoteStatus::Active,
                View::Pinned => note.status == NoteStatus::Active && note.pinned,
                View::Archive => note.status == NoteStatus::Archived,
                View::Trash => note.status == NoteStatus::Deleted,
                View::Inbox => false,
            };
            let haystack =
                format!("{} {} {}", note.title, note.body, note.tags.join(" ")).to_lowercase();
            in_view
                && (tag.is_empty() || note.tags.iter().any(|candidate| candidate == tag))
                && terms.iter().all(|term| haystack.contains(term))
        })
        .cloned()
        .collect();
    matches.sort_by(|a, b| {
        b.pinned.cmp(&a.pinned).then_with(|| {
            // Imported timestamps can use different UTC offsets.
            let a_time = DateTime::parse_from_rfc3339(&a.updated_at).ok();
            let b_time = DateTime::parse_from_rfc3339(&b.updated_at).ok();
            b_time.cmp(&a_time)
        })
    });
    matches
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backup {
    pub version: u32,
    pub notes: Vec<Note>,
}

impl Backup {
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.len() > 20 * 1024 * 1024 {
            return Err("Backup exceeds the 20 MB limit.".into());
        }
        let backup: Self =
            serde_json::from_str(raw).map_err(|error| format!("Invalid backup: {error}"))?;
        if backup.version != 1 || backup.notes.len() > 10_000 {
            return Err(
                "Select a version 1 Fieldnotes backup containing no more than 10,000 notes.".into(),
            );
        }
        let mut ids = HashSet::new();
        for note in &backup.notes {
            note.validate()?;
            if !ids.insert(&note.id) {
                return Err("The backup contains duplicate note IDs.".into());
            }
        }
        Ok(backup)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Handoff {
    pub id: String,
    pub note_id: String,
    pub title: String,
    pub body: String,
    pub agent: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub note_id: String,
    pub title: String,
    pub body: String,
    pub agent: String,
    pub created_at: String,
    pub status: String,
    pub received_at: String,
}

impl InboxItem {
    pub fn from_handoff(handoff: Handoff, status: String, received_at: String) -> Self {
        Self {
            id: handoff.id,
            note_id: handoff.note_id,
            title: handoff.title,
            body: handoff.body,
            agent: handoff.agent,
            created_at: handoff.created_at,
            status,
            received_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_valid_and_unique_and_round_trip_the_browser_schema() {
        let note = Note::new();
        note.validate().unwrap();
        assert_ne!(note.id, Note::new().id);
        let json = serde_json::to_value(&note).unwrap();
        assert!(json.get("createdAt").is_some());
        assert_eq!(json["status"], "active");
        assert_eq!(json["color"], "paper");
        assert_eq!(serde_json::from_value::<Note>(json).unwrap(), note);
    }

    #[test]
    fn import_rejects_duplicate_ids_bad_dates_and_unknown_enums() {
        let note = Note::new();
        let duplicate = serde_json::to_string(&Backup {
            version: 1,
            notes: vec![note.clone(), note.clone()],
        })
        .unwrap();
        assert!(Backup::parse(&duplicate).is_err());
        let mut bad = note;
        bad.created_at = "yesterday".into();
        assert!(bad.validate().is_err());
        let invalid = serde_json::to_string(&Backup {
            version: 1,
            notes: vec![Note::new()],
        })
        .unwrap()
        .replace("\"paper\"", "\"blue\"");
        assert!(Backup::parse(&invalid).is_err());
    }

    #[test]
    fn tags_are_trimmed_deduplicated_and_bounded() {
        assert_eq!(
            parse_tags("#work, idea,work, ,#home"),
            vec!["work", "idea", "home"]
        );
        assert_eq!(parse_tags(&"é".repeat(45))[0].chars().count(), 40);
        assert_eq!(
            parse_tags(&(0..30).map(|i| i.to_string()).collect::<Vec<_>>().join(",")).len(),
            20
        );
    }

    #[test]
    fn task_toggles_preserve_other_lines_and_support_uppercase_and_indentation() {
        let mut note = Note::new();
        note.body = "  - [X] One\n- [ ] Two\nPlain\n".into();
        note.toggle_task(0);
        assert_eq!(note.body, "  - [ ] One\n- [ ] Two\nPlain\n");
        note.toggle_task(1);
        note.toggle_task(2);
        assert_eq!(note.body, "  - [ ] One\n- [x] Two\n- [x] Plain\n");
    }

    #[test]
    fn view_search_and_tags_combine_without_leaking_archive_or_trash() {
        let mut a = Note::new();
        a.title = "Build Rust".into();
        a.tags = vec!["work".into()];
        a.pinned = true;
        let mut b = a.clone();
        b.id = Uuid::new_v4().to_string();
        b.status = NoteStatus::Archived;
        let notes = vec![a.clone(), b.clone()];
        assert_eq!(
            filter_notes(&notes, View::Pinned, "rust WORK", "work"),
            vec![a]
        );
        assert_eq!(filter_notes(&notes, View::Archive, "build", ""), vec![b]);
        assert!(filter_notes(&notes, View::All, "missing", "").is_empty());
    }

    #[test]
    fn markdown_contains_agent_instructions_and_exact_user_text() {
        let mut note = Note::new();
        note.title = "Ship it".into();
        note.body = "<script>stay inert</script>".into();
        let text = note.markdown("codex", "Review tests");
        assert!(text.contains(&note.body));
        assert!(text.contains("Assigned to: codex"));
        assert!(text.contains(&note.id));
        assert!(text.contains("Instructions:\nReview tests"));
    }
}
