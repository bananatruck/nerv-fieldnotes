//! Runs the production GTK widgets and database in a caller-selected test directory.
use crate::{
    model::{Color, Note, NoteStatus, View},
    ui::{self, Ui},
};
use gtk::{glib, prelude::*};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
pub static FAILED: AtomicBool = AtomicBool::new(false);
fn check(value: bool, message: &str) -> Result<(), String> {
    if value {
        println!("PASS {message}");
        Ok(())
    } else {
        Err(message.into())
    }
}
fn finish(ui: &Ui, result: Result<(), String>) {
    if let Err(error) = result {
        FAILED.store(true, Ordering::SeqCst);
        eprintln!("FAIL {error}");
    } else {
        println!("Native GTK smoke suite passed");
    }
    ui.app.quit();
}
pub fn schedule(ui: Rc<Ui>) {
    let start = (|| -> Result<(), String> {
        ui.launch_recent()?;
        check(
            ui.app.windows().len() == 1,
            "startup registers only the sticky with the desktop",
        )?;
        check(
            !ui.window.is_visible(),
            "launch keeps the full board hidden",
        )?;
        check(
            ui.editors.borrow().len() == 1,
            "launch opens exactly one compact sticky",
        )?;
        ui.launch_recent()?;
        check(
            ui.editors.borrow().len() == 1,
            "relaunch focuses the existing blank sticky",
        )?;
        let editor = ui.editors.borrow()[0].clone();
        editor.title.set_text("Native acceptance note");
        editor
            .body
            .buffer()
            .set_text("- [ ] Ship a native sticky\n- [x] Keep data local");
        editor.tags.set_text("rust, personal");
        editor.pin.set_active(true);
        editor.checklist.set_active(true);
        editor.color.set_selected(1);
        Ok(())
    })();
    if let Err(error) = start {
        finish(&ui, Err(error));
        return;
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
        let result = exercise(&ui);
        if let Err(error) = result {
            finish(&ui, Err(error));
            return;
        }
        let u = ui.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            let result = (|| -> Result<(), String> {
                let directory = PathBuf::from(
                    std::env::var_os("FIELDNOTES_SCREENSHOT_DIR")
                        .unwrap_or_else(|| "artifacts".into()),
                );
                std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                ui::screenshot(&u.window, &directory.join("native-board.png"))?;
                let editor = u
                    .editors
                    .borrow()
                    .last()
                    .cloned()
                    .ok_or("No sticky for screenshot")?;
                ui::screenshot(&editor.window, &directory.join("native-sticky.png"))?;
                check(
                    editor.body.width() >= editor.window.width() - 16,
                    "writing canvas uses the sticky width with at most 8px per side",
                )?;
                check(
                    editor.body.height() >= editor.window.height() - 52,
                    "header and all vertical chrome use at most 52px",
                )?;
                check(true, "actual GTK board and sticky render to screenshots")?;
                Ok(())
            })();
            finish(&u, result);
        });
    });
}
fn exercise(ui: &Rc<Ui>) -> Result<(), String> {
    let notes = ui.store.load_notes()?;
    let note = notes
        .iter()
        .find(|n| n.title == "Native acceptance note")
        .ok_or("Autosave did not persist the actual GTK text")?
        .clone();
    check(
        note.color == Color::Amber
            && note.pinned
            && note.checklist
            && note.tags == ["rust", "personal"],
        "debounced GTK controls save directly to SQLite",
    )?;
    let editor = ui.editors.borrow()[0].clone();
    editor.board.emit_clicked();
    check(
        ui.window.is_visible(),
        "sticky dots menu All notes opens the board",
    )?;
    ui.search.set_text("does-not-exist");
    ui.refresh();
    check(
        ui.flow.child_at_index(1).is_none(),
        "search renders only its empty state",
    )?;
    ui.search.set_text("");
    ui.refresh();
    ui.move_note(&note, NoteStatus::Archived);
    check(
        ui.store
            .load_notes()?
            .iter()
            .find(|n| n.id == note.id)
            .is_some_and(|n| n.status == NoteStatus::Archived),
        "archive writes the durable note state",
    )?;
    ui.undo_move();
    check(
        ui.store
            .load_notes()?
            .iter()
            .find(|n| n.id == note.id)
            .is_some_and(|n| n.status == NoteStatus::Active),
        "undo restores the durable note state",
    )?;
    ui.move_note(&note, NoteStatus::Deleted);
    ui.view.set(View::Trash);
    ui.refresh();
    ui.undo_move();
    ui.view.set(View::All);
    ui.refresh();
    let handoff = ui
        .store
        .handoff(&note, "native-test-agent", "Verify native delivery")?;
    check(
        ui.store
            .inbox()?
            .iter()
            .any(|i| i.id == handoff.id && i.body.contains("Verify native delivery")),
        "handoff reaches the actual durable agent inbox",
    )?;
    ui.store.acknowledge(&handoff.id)?;
    check(
        ui.store
            .inbox()?
            .iter()
            .any(|i| i.id == handoff.id && i.status == "acknowledged"),
        "agent receipt persists",
    )?;
    // Board metadata changes must not be undone by a later sticky autosave.
    editor
        .body
        .buffer()
        .set_text("Latest typing before board action");
    ui.move_note(&note, NoteStatus::Archived);
    editor.body.buffer().set_text("Typing after archive");
    check(editor.flush(), "archived sticky edits save successfully")?;
    let archived = ui
        .store
        .load_notes()?
        .into_iter()
        .find(|n| n.id == note.id)
        .ok_or("Missing archived note")?;
    check(
        archived.status == NoteStatus::Archived && archived.body == "Typing after archive",
        "typing in an open sticky cannot resurrect an archived note",
    )?;
    ui.undo_move();
    ui.change_note(&note.id, |n| n.pinned = false);
    editor.body.buffer().set_text("Typing after unpin");
    editor.flush();
    check(
        ui.store
            .load_notes()?
            .iter()
            .find(|n| n.id == note.id)
            .is_some_and(|n| !n.pinned),
        "sticky autosave preserves a pin change made from board",
    )?;
    ui.view.set(View::Trash);
    ui.search.set_text("old filter");
    editor.board.emit_clicked();
    check(
        ui.view.get() == View::All && ui.search.text().is_empty(),
        "All notes menu clears old board filters",
    )?;
    let backup = ui.store.export_backup()?;
    check(
        backup.contains("Native acceptance note"),
        "backup contains the edited native note",
    )?;
    check(
        ui.store.import_backup("{bad").is_err()
            && ui.store.load_notes()?.iter().any(|n| n.id == note.id),
        "invalid imports preserve the real database",
    )?;
    let mut recent = Note::new();
    recent.title = "Most recent sticky".into();
    recent.body="A little room to think.\n\nThree dots opens all your notes.\nEverything saves as you type.".into();
    recent.color = Color::Paper;
    ui.store.save_note(&recent)?;
    check(
        ui.latest()?.is_some_and(|n| n.id == recent.id),
        "startup chooses most recently edited active note",
    )?;
    let editor_weak = Rc::downgrade(&editor);
    editor.save.emit_clicked();
    check(
        !editor.window.is_visible(),
        "Done closes the sticky without hiding the open board",
    )?;
    drop(editor);
    check(
        editor_weak.upgrade().is_none(),
        "closed sticky releases its Rust and GTK event owners",
    )?;
    ui.launch_recent()?;
    let current = ui
        .editors
        .borrow()
        .last()
        .cloned()
        .ok_or("Most recent sticky did not open")?;
    check(
        current.note.borrow().id == recent.id,
        "reopening shows the most recent sticky",
    )?;
    check(
        current.window.default_width() == 320 && current.window.default_height() == 200,
        "sticky opens at compact desktop dimensions",
    )?;
    check(
        !current.title.is_mapped() && !current.details.is_expanded(),
        "title and note details stay off the writing canvas",
    )?;
    check(
        current.menu.popover().is_some(),
        "metadata and Save remain available in the dots menu",
    )?;
    ui.refresh();
    Ok(())
}
