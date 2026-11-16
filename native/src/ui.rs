use crate::model::{Color, Note, NoteStatus, View, filter_notes, parse_tags};
use crate::store::Store;
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
};

pub struct Ui {
    pub app: gtk::Application,
    pub window: gtk::ApplicationWindow,
    pub store: Store,
    pub search: gtk::SearchEntry,
    pub flow: gtk::FlowBox,
    pub status: gtk::Label,
    pub view: Cell<View>,
    tag: RefCell<String>,
    pub editors: RefCell<Vec<Rc<Editor>>>,
    undo: RefCell<Option<(String, NoteStatus)>>,
    nav: Vec<(View, gtk::Button)>,
}

pub struct Editor {
    pub window: gtk::Window,
    pub title: gtk::Entry,
    pub body: gtk::TextView,
    pub tags: gtk::Entry,
    pub pin: gtk::CheckButton,
    pub checklist: gtk::CheckButton,
    pub color: gtk::DropDown,
    pub save: gtk::Button,
    pub board: gtk::Button,
    pub forward: gtk::Button,
    pub note: RefCell<Note>,
    status: gtk::Label,
    timer: RefCell<Option<glib::SourceId>>,
    ui: Rc<Ui>,
    dirty: Cell<bool>,
    syncing: Cell<bool>,
    persisted: Cell<bool>,
}

fn text(value: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(value));
    label.set_xalign(0.0);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}
fn button(value: &str, class: &str) -> gtk::Button {
    let b = gtk::Button::with_label(value);
    if !class.is_empty() {
        b.add_css_class(class);
    }
    b
}
fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}
fn margins(widget: &impl IsA<gtk::Widget>, value: i32) {
    widget.set_margin_top(value);
    widget.set_margin_bottom(value);
    widget.set_margin_start(value);
    widget.set_margin_end(value);
}
fn buffer_text(view: &gtk::TextView) -> String {
    let b = view.buffer();
    b.text(&b.start_iter(), &b.end_iter(), true).to_string()
}

impl Ui {
    pub fn new(app: &gtk::Application, store: Store) -> Rc<Self> {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(include_str!("../style.css"));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("NERV Fieldnotes · All notes")
            .default_width(1220)
            .default_height(780)
            .build();
        window.add_css_class("fieldnotes");
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&text("NERV / PERSONAL OPERATIONS", "section-label")));
        window.set_titlebar(Some(&header));
        let root = hbox(0);
        window.set_child(Some(&root));
        let sidebar = vbox(14);
        sidebar.add_css_class("sidebar");
        sidebar.set_width_request(200);
        sidebar.append(&text("NERV\nFIELDNOTES", "brand"));
        sidebar.append(&text("NATIVE MEMORY SYSTEM", "brand-subtitle"));
        sidebar.append(&text("WORKSPACE / 01", "section-label"));
        let mut nav = Vec::new();
        for (view, label) in [
            (View::All, "All notes"),
            (View::Pinned, "Pinned"),
            (View::Archive, "Archive"),
            (View::Trash, "Trash"),
            (View::Inbox, "Agent inbox"),
        ] {
            let b = button(label, "nav-button");
            let l = text(label, "");
            b.set_child(Some(&l));
            sidebar.append(&b);
            nav.push((view, b));
        }
        let spacer = vbox(0);
        spacer.set_vexpand(true);
        sidebar.append(&spacer);
        sidebar.append(&text("RUST + GTK4\nSAVED ON THIS DEVICE", "sidebar-footer"));
        let import = button("Import backup", "");
        let export = button("Export backup", "");
        let help = button("Keyboard shortcuts", "");
        sidebar.append(&import);
        sidebar.append(&export);
        sidebar.append(&help);
        root.append(&sidebar);
        let workspace = vbox(16);
        workspace.add_css_class("workspace");
        workspace.set_hexpand(true);
        root.append(&workspace);
        workspace.append(&text("EXTERNAL MEMORY / PERSONAL OPERATIONS", "topline"));
        workspace.append(&text("FIELDNOTES", "hero-title"));
        workspace.append(&text("Clear your head. Leave a note.", "hero-subtitle"));
        let toolbar = hbox(10);
        toolbar.add_css_class("toolbar");
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search notes and tags"));
        search.set_hexpand(true);
        toolbar.append(&search);
        let undo = button("Undo move", "");
        toolbar.append(&undo);
        let new = button("+ New note", "primary");
        toolbar.append(&new);
        workspace.append(&toolbar);
        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_min_children_per_line(1);
        flow.set_max_children_per_line(3);
        flow.set_column_spacing(16);
        flow.set_row_spacing(16);
        flow.set_valign(gtk::Align::Start);
        flow.set_homogeneous(true);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&flow)
            .build();
        workspace.append(&scroll);
        let status = text(
            &format!("LOCAL SQLITE / {}", store.path().display()),
            "status",
        );
        status.set_wrap(true);
        workspace.append(&status);
        let ui = Rc::new(Self {
            app: app.clone(),
            window,
            store,
            search,
            flow,
            status,
            view: Cell::new(View::All),
            tag: RefCell::new(String::new()),
            editors: RefCell::new(Vec::new()),
            undo: RefCell::new(None),
            nav,
        });
        for (view, b) in &ui.nav {
            let u = ui.clone();
            let view = *view;
            b.connect_clicked(move |_| {
                u.view.set(view);
                u.tag.borrow_mut().clear();
                u.refresh();
            });
        }
        {
            let u = ui.clone();
            ui.search.connect_search_changed(move |_| u.refresh());
        }
        {
            let u = ui.clone();
            new.connect_clicked(move |_| {
                u.open_note(None);
            });
        }
        {
            let u = ui.clone();
            undo.connect_clicked(move |_| u.undo_move());
        }
        {
            let u = ui.clone();
            import.connect_clicked(move |_| u.import_dialog());
        }
        {
            let u = ui.clone();
            export.connect_clicked(move |_| u.export_dialog());
        }
        {
            let u = ui.clone();
            help.connect_clicked(move |_| u.help());
        }
        {
            let u = ui.clone();
            ui.window.connect_close_request(move |_| {
                u.window.set_visible(false);
                if !u
                    .app
                    .windows()
                    .iter()
                    .any(|w| w.is_visible() && *w != u.window)
                {
                    u.app.quit();
                }
                glib::Propagation::Stop
            });
        }
        ui.install_actions();
        ui.refresh();
        ui
    }
    pub fn message(&self, value: &str) {
        self.status.set_text(value);
    }
    pub fn show_board(self: &Rc<Self>) {
        self.window.present();
        self.refresh();
    }
    pub fn latest(&self) -> Result<Option<Note>, String> {
        let mut notes = self.store.load_notes()?;
        notes.retain(|n| n.status == NoteStatus::Active);
        Ok(notes
            .into_iter()
            .max_by_key(|n| chrono::DateTime::parse_from_rfc3339(&n.updated_at).ok()))
    }
    pub fn open_note(self: &Rc<Self>, note: Option<Note>) -> Rc<Editor> {
        let mut note = note.unwrap_or_default();
        if let Some(existing) = self
            .editors
            .borrow()
            .iter()
            .find(|e| e.note.borrow().id == note.id)
        {
            existing.window.present();
            return existing.clone();
        }
        match self.store.load_draft(&note.id) {
            Ok(Some(draft)) => note = draft,
            Ok(None) => {}
            Err(e) => self.message(&e),
        }
        let editor = Editor::new(self.clone(), note);
        self.editors.borrow_mut().push(editor.clone());
        editor.window.present();
        editor.body.grab_focus();
        editor
    }
    pub fn launch_recent(self: &Rc<Self>) -> Result<(), String> {
        if let Some(editor) = self
            .editors
            .borrow()
            .iter()
            .rev()
            .find(|e| e.dirty.get() && e.note.borrow().status == NoteStatus::Active)
        {
            editor.window.present();
            return Ok(());
        }

        let drafts = self.store.load_drafts()?;
        let mut recent = self.latest()?;
        if let Some(draft) = drafts
            .into_iter()
            .max_by_key(|n| chrono::DateTime::parse_from_rfc3339(&n.updated_at).ok())
            && recent.as_ref().is_none_or(|n| {
                chrono::DateTime::parse_from_rfc3339(&draft.updated_at).ok()
                    >= chrono::DateTime::parse_from_rfc3339(&n.updated_at).ok()
            })
        {
            recent = Some(draft);
        }
        if recent.is_none()
            && let Some(editor) = self.editors.borrow().last()
        {
            editor.window.present();
            return Ok(());
        }
        self.open_note(recent);
        Ok(())
    }
    pub fn refresh(self: &Rc<Self>) {
        if !self.window.is_visible() {
            return;
        }
        while let Some(child) = self.flow.first_child() {
            self.flow.remove(&child);
        }
        for (view, b) in &self.nav {
            if *view == self.view.get() {
                b.add_css_class("selected");
            } else {
                b.remove_css_class("selected");
            }
        }
        if self.view.get() == View::Inbox {
            self.render_inbox();
            return;
        }
        match self.store.load_notes() {
            Ok(notes) => {
                let notes = filter_notes(
                    &notes,
                    self.view.get(),
                    &self.search.text(),
                    &self.tag.borrow(),
                );
                if notes.is_empty() {
                    let empty = vbox(12);
                    margins(&empty, 24);
                    empty.append(&text("Room for a thought.", "empty-title"));
                    let hint = text("Use + New note, or change your search.", "");
                    hint.set_wrap(true);
                    empty.append(&hint);
                    self.flow.insert(&empty, -1);
                }
                for note in notes {
                    self.flow.insert(&self.card(note), -1);
                }
            }
            Err(e) => self.message(&format!("Could not read notes: {e}")),
        }
    }
    fn card(self: &Rc<Self>, note: Note) -> gtk::Box {
        let card = vbox(12);
        card.add_css_class("note-card");
        card.add_css_class(note.color.as_str());
        card.set_width_request(250);
        let top = hbox(8);
        let category = text(
            if note.checklist {
                "CHECKLIST"
            } else {
                "FIELD NOTE"
            },
            "note-meta",
        );
        category.set_hexpand(true);
        top.append(&category);
        let pin = button(if note.pinned { "◆" } else { "◇" }, "pin-button");
        pin.set_tooltip_text(Some("Toggle pin"));
        {
            let u = self.clone();
            let n = note.clone();
            pin.connect_clicked(move |_| {
                u.change_note(&n.id, |note| note.pinned = !note.pinned);
            });
        }
        top.append(&pin);
        card.append(&top);
        let title = button("", "note-title");
        let label = text(
            if note.title.is_empty() {
                "Untitled note"
            } else {
                &note.title
            },
            "",
        );
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_child(Some(&label));
        {
            let u = self.clone();
            let n = note.clone();
            title.connect_clicked(move |_| {
                u.open_note(Some(n.clone()));
            });
        }
        card.append(&title);
        if note.checklist {
            for (index, line) in note.body.lines().enumerate().take(6) {
                let checked = line.trim_start().starts_with("- [x]")
                    || line.trim_start().starts_with("- [X]");
                let label = line
                    .trim_start()
                    .strip_prefix("- [ ] ")
                    .or_else(|| line.trim_start().strip_prefix("- [x] "))
                    .or_else(|| line.trim_start().strip_prefix("- [X] "))
                    .unwrap_or(line);
                let task = gtk::CheckButton::with_label(label);
                task.set_active(checked);
                task.set_sensitive(note.status == NoteStatus::Active);
                {
                    let u = self.clone();
                    let n = note.clone();
                    task.connect_toggled(move |_| {
                        u.change_note(&n.id, |note| note.toggle_task(index));
                    });
                }
                card.append(&task);
            }
        } else {
            let preview = text(&note.body, "note-preview");
            preview.set_wrap(true);
            preview.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            preview.set_lines(6);
            preview.set_ellipsize(gtk::pango::EllipsizeMode::End);
            card.append(&preview);
        }
        let tags = text(
            &note
                .tags
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join("  "),
            "note-tag",
        );
        tags.set_wrap(true);
        if !note.tags.is_empty() {
            card.append(&tags);
        }
        let actions = hbox(3);
        actions.add_css_class("note-actions");
        let date = text(
            &note.updated_at.chars().take(10).collect::<String>(),
            "note-meta",
        );
        date.set_hexpand(true);
        actions.append(&date);
        if note.status == NoteStatus::Deleted {
            let restore = button("Restore", "");
            {
                let u = self.clone();
                let n = note.clone();
                restore.connect_clicked(move |_| u.move_note(&n, NoteStatus::Active));
            }
            actions.append(&restore);
            let delete = button("Delete", "danger");
            {
                let u = self.clone();
                let id = note.id.clone();
                delete.connect_clicked(move |_| u.confirm_delete(&id));
            }
            actions.append(&delete);
        } else {
            let forward = button("↗", "");
            forward.set_tooltip_text(Some("Forward to agent"));
            {
                let u = self.clone();
                let n = note.clone();
                forward.connect_clicked(move |_| u.forward_dialog(n.clone()));
            }
            actions.append(&forward);
            let duplicate = button("⧉", "");
            duplicate.set_tooltip_text(Some("Duplicate note"));
            {
                let u = self.clone();
                let n = note.clone();
                duplicate.connect_clicked(move |_| {
                    let mut copy = n.clone();
                    copy.id = uuid::Uuid::new_v4().to_string();
                    copy.title = format!("{} (copy)", copy.title);
                    copy.status = NoteStatus::Active;
                    copy.touch();
                    u.write_note(copy);
                });
            }
            actions.append(&duplicate);
            let archive = button(
                if note.status == NoteStatus::Archived {
                    "↶"
                } else {
                    "▤"
                },
                "",
            );
            archive.set_tooltip_text(Some("Archive or restore"));
            {
                let u = self.clone();
                let n = note.clone();
                archive.connect_clicked(move |_| {
                    u.move_note(
                        &n,
                        if n.status == NoteStatus::Archived {
                            NoteStatus::Active
                        } else {
                            NoteStatus::Archived
                        },
                    )
                });
            }
            actions.append(&archive);
            let trash = button("×", "");
            trash.set_tooltip_text(Some("Move to trash"));
            {
                let u = self.clone();
                let n = note.clone();
                trash.connect_clicked(move |_| u.move_note(&n, NoteStatus::Deleted));
            }
            actions.append(&trash);
        }
        card.append(&actions);
        card
    }
    pub fn write_note(self: &Rc<Self>, note: Note) {
        match self.store.save_note(&note) {
            Ok(()) => {
                self.message("Saved on this device");
                self.refresh();
            }
            Err(e) => self.message(&e),
        }
    }
    pub fn change_note(self: &Rc<Self>, id: &str, change: impl FnOnce(&mut Note)) -> bool {
        let open = self
            .editors
            .borrow()
            .iter()
            .filter(|e| e.note.borrow().id == id)
            .cloned()
            .collect::<Vec<_>>();
        for editor in &open {
            if !editor.flush() {
                self.message("Save the open sticky before changing it from the board.");
                return false;
            }
        }
        let result = self.store.load_notes().and_then(|notes| {
            notes
                .into_iter()
                .find(|n| n.id == id)
                .ok_or_else(|| "Note no longer exists".to_string())
        });
        match result {
            Ok(mut note) => {
                change(&mut note);
                note.touch();
                match self.store.save_note(&note) {
                    Ok(()) => {
                        for editor in open {
                            editor.replace_note(note.clone());
                        }
                        self.refresh();
                        true
                    }
                    Err(e) => {
                        self.message(&e);
                        false
                    }
                }
            }
            Err(e) => {
                self.message(&e);
                false
            }
        }
    }
    pub fn move_note(self: &Rc<Self>, note: &Note, status: NoteStatus) {
        let prior = self
            .store
            .load_notes()
            .ok()
            .and_then(|notes| notes.into_iter().find(|n| n.id == note.id))
            .map(|n| n.status)
            .unwrap_or(note.status);
        if self.change_note(&note.id, |n| n.status = status) {
            *self.undo.borrow_mut() = Some((note.id.clone(), prior));
            self.message("Note moved. Ctrl+Z to undo.");
        }
    }
    pub fn undo_move(self: &Rc<Self>) {
        let undo = self.undo.borrow().clone();
        if let Some((id, status)) = undo
            && self.change_note(&id, |n| n.status = status)
        {
            self.undo.borrow_mut().take();
            self.message("Move undone");
        }
    }
    fn confirm_delete(self: &Rc<Self>, id: &str) {
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message("Delete note forever?")
            .detail("This cannot be undone. Export a backup if you may need this note.")
            .buttons(["Cancel", "Delete forever"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let u = self.clone();
        let id = id.to_string();
        dialog.choose(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |choice| {
                if choice == Ok(1) {
                    match u.store.delete_note(&id) {
                        Ok(()) => {
                            u.refresh();
                            u.message("Note deleted permanently");
                        }
                        Err(e) => u.message(&e),
                    }
                }
            },
        );
    }
    fn render_inbox(self: &Rc<Self>) {
        match self.store.inbox() {
            Ok(items) => {
                if items.is_empty() {
                    self.flow.insert(
                        &text("No handoffs yet. Forward a note to begin.", "empty-title"),
                        -1,
                    );
                }
                for item in items {
                    let card = vbox(10);
                    card.add_css_class("inbox-card");
                    card.append(&text(&item.title, "note-title"));
                    card.append(&text(
                        &format!("{} / {}", item.agent, item.status),
                        "note-meta",
                    ));
                    let body = text(&item.body, "note-preview");
                    body.set_wrap(true);
                    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    body.set_lines(10);
                    body.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    card.append(&body);
                    if item.status == "pending" {
                        let ack = button("Acknowledge receipt", "");
                        let u = self.clone();
                        let id = item.id.clone();
                        ack.connect_clicked(move |_| match u.store.acknowledge(&id) {
                            Ok(()) => u.refresh(),
                            Err(e) => u.message(&e),
                        });
                        card.append(&ack);
                    }
                    self.flow.insert(&card, -1);
                }
            }
            Err(e) => self.message(&e),
        }
    }
    pub fn forward_dialog(self: &Rc<Self>, note: Note) {
        let window = gtk::Window::builder()
            .application(&self.app)
            .title("Forward to agent")
            .default_width(520)
            .default_height(540)
            .build();
        window.add_css_class("fieldnotes");
        let root = vbox(12);
        margins(&root, 20);
        window.set_child(Some(&root));
        root.append(&text("AGENT HANDOFF", "section-label"));
        let agent = gtk::Entry::builder()
            .placeholder_text("Agent name")
            .text("Agent")
            .build();
        root.append(&agent);
        let instructions = gtk::Entry::builder()
            .placeholder_text("Extra instructions, optional")
            .build();
        root.append(&instructions);
        let preview = gtk::TextView::new();
        preview.set_editable(false);
        preview.set_wrap_mode(gtk::WrapMode::WordChar);
        preview.buffer().set_text(&note.markdown("Agent", ""));
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .child(&preview)
            .build();
        root.append(&scroll);
        for entry in [&agent, &instructions] {
            let p = preview.clone();
            let a = agent.clone();
            let i = instructions.clone();
            let n = note.clone();
            entry.connect_changed(move |_| p.buffer().set_text(&n.markdown(&a.text(), &i.text())));
        }
        let hint = text(
            "Queues a snapshot. Your agents read the inbox; note text is never executed.",
            "status",
        );
        hint.set_wrap(true);
        root.append(&hint);
        let actions = hbox(8);
        let copy = button("Copy", "");
        let download = button("Save .md", "");
        let send = button("Send to inbox ↗", "primary");
        actions.append(&copy);
        actions.append(&download);
        actions.append(&send);
        root.append(&actions);
        {
            let p = preview.clone();
            let h = hint.clone();
            copy.connect_clicked(move |_| {
                if let Some(d) = gdk::Display::default() {
                    d.clipboard().set_text(&buffer_text(&p));
                    h.set_text("Handoff copied");
                }
            });
        }
        {
            let parent = window.clone();
            let p = preview.clone();
            let h = hint.clone();
            download.connect_clicked(move |_| {
                let dialog = gtk::FileDialog::builder()
                    .title("Save handoff")
                    .initial_name("fieldnote.md")
                    .build();
                let content = buffer_text(&p);
                let h = h.clone();
                dialog.save(Some(&parent), None::<&gio::Cancellable>, move |result| {
                    if let Ok(file) = result {
                        match file.replace_contents(
                            content.as_bytes(),
                            None,
                            false,
                            gio::FileCreateFlags::PRIVATE,
                            None::<&gio::Cancellable>,
                        ) {
                            Ok(_) => h.set_text("Markdown handoff saved"),
                            Err(e) => h.set_text(&e.to_string()),
                        }
                    }
                });
            });
        }
        {
            let u = self.clone();
            let h = hint.clone();
            let n = note.clone();
            let w = window.clone();
            send.connect_clicked(move |_| {
                match u.store.handoff(&n, &agent.text(), &instructions.text()) {
                    Ok(_) => {
                        u.message("Handoff queued in local inbox");
                        u.refresh();
                        w.close();
                    }
                    Err(e) => h.set_text(&e),
                }
            });
        }
        {
            let u = self.clone();
            window.connect_close_request(move |w| {
                if !u
                    .app
                    .windows()
                    .iter()
                    .any(|other| other.is_visible() && other != w)
                {
                    u.app.quit();
                }
                glib::Propagation::Proceed
            });
        }
        window.present();
    }
    fn import_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Import Fieldnotes JSON backup")
            .build();
        let u = self.clone();
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| {
                if let Ok(file) = result {
                    let result = file
                        .load_contents(None::<&gio::Cancellable>)
                        .map_err(|e| e.to_string())
                        .and_then(|(bytes, _)| {
                            if bytes.len() > 20 * 1024 * 1024 {
                                return Err("Backup exceeds 20 MB".into());
                            }
                            let content = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
                            u.store.import_backup(content)
                        });
                    match result {
                        Ok(count) => {
                            u.refresh();
                            u.message(&format!(
                                "Imported {count} notes. Newer local edits were kept."
                            ));
                        }
                        Err(e) => u.message(&format!("Import failed: {e}")),
                    }
                }
            },
        );
    }
    fn export_dialog(self: &Rc<Self>) {
        match self.store.export_backup() {
            Ok(content) => {
                let dialog = gtk::FileDialog::builder()
                    .title("Export all notes")
                    .initial_name("fieldnotes-backup.json")
                    .build();
                let u = self.clone();
                dialog.save(
                    Some(&self.window),
                    None::<&gio::Cancellable>,
                    move |result| {
                        if let Ok(file) = result {
                            match file.replace_contents(
                                content.as_bytes(),
                                None,
                                false,
                                gio::FileCreateFlags::PRIVATE,
                                None::<&gio::Cancellable>,
                            ) {
                                Ok(_) => u.message("Backup exported"),
                                Err(e) => u.message(&format!("Export failed: {e}")),
                            }
                        }
                    },
                );
            }
            Err(e) => self.message(&e),
        }
    }
    fn help(&self) {
        let dialog=gtk::AlertDialog::builder().message("Keyboard shortcuts").detail("Ctrl+N  New sticky note\nCtrl+K  Open board and search\nCtrl+Enter  Save and close note\nCtrl+S  Save now\nCtrl+Z  Undo move on the board\nEscape  Close sticky or hide board\nF1  Show shortcuts\n\nNotes save automatically while you type. Use the three dots on a sticky to open all notes.").buttons(["Close"]).build();
        dialog.show(Some(&self.window));
    }
    fn install_actions(self: &Rc<Self>) {
        for (name, keys) in [
            ("new", vec!["<Primary>n"]),
            ("search", vec!["<Primary>k", "<Primary>slash"]),
            ("help", vec!["F1"]),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let u = self.clone();
            action.connect_activate(move |_, _| match name {
                "new" => {
                    u.open_note(None);
                }
                "search" => {
                    u.show_board();
                    u.search.grab_focus();
                }
                _ => u.help(),
            });
            self.app.add_action(&action);
            self.app
                .set_accels_for_action(&format!("app.{name}"), &keys);
        }
        let ctrl = gtk::EventControllerKey::new();
        let u = self.clone();
        ctrl.connect_key_pressed(move |_, key, _, mods| {
            if key == gdk::Key::Escape {
                u.window.close();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::z
                && mods.contains(gdk::ModifierType::CONTROL_MASK)
                && gtk::prelude::GtkWindowExt::focus(&u.window)
                    .is_none_or(|w| !w.is::<gtk::Editable>() && !w.is::<gtk::TextView>())
            {
                u.undo_move();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(ctrl);
    }
}

impl Editor {
    fn new(ui: Rc<Ui>, note: Note) -> Rc<Self> {
        let window = gtk::Window::builder()
            .application(&ui.app)
            .title(if note.title.is_empty() {
                "NERV Fieldnote"
            } else {
                &note.title
            })
            .default_width(370)
            .default_height(340)
            .build();
        if let Ok(Some(value)) = ui.store.get_setting(&format!("note-size:{}", note.id))
            && let Some((w, h)) = value
                .split_once(',')
                .and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))
            && (260..=1600).contains(&w)
            && (220..=1400).contains(&h)
        {
            window.set_default_size(w, h);
        }
        window.add_css_class("sticky-window");
        window.add_css_class(note.color.as_str());
        let header = gtk::HeaderBar::new();
        header.set_decoration_layout(Some(":close"));
        header.set_title_widget(Some(&text("FIELD NOTE", "section-label")));
        let new = button("+", "");
        new.set_tooltip_text(Some("New sticky note · Ctrl+N"));
        header.pack_start(&new);
        let menu = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Note menu and all notes")
            .build();
        header.pack_end(&menu);
        window.set_titlebar(Some(&header));
        let popover = gtk::Popover::new();
        let menu_box = vbox(4);
        margins(&menu_box, 6);
        let board = button("All notes", "");
        menu_box.append(&board);
        let forward = button("Forward to agent", "");
        menu_box.append(&forward);
        let options = button("Note options", "");
        menu_box.append(&options);
        let archive = button("Archive note", "");
        menu_box.append(&archive);
        let trash = button("Move to trash", "danger");
        menu_box.append(&trash);
        popover.set_child(Some(&menu_box));
        menu.set_popover(Some(&popover));
        let root = vbox(6);
        root.add_css_class("sticky-content");
        root.add_css_class("editor");
        margins(&root, 10);
        window.set_child(Some(&root));
        let title = gtk::Entry::builder()
            .placeholder_text("Untitled")
            .text(&note.title)
            .max_length(500)
            .build();
        title.add_css_class("sticky-title");
        root.append(&title);
        let body = gtk::TextView::new();
        body.set_wrap_mode(gtk::WrapMode::WordChar);
        body.add_css_class("sticky-body");
        body.buffer().set_text(&note.body);
        body.set_top_margin(4);
        body.set_bottom_margin(4);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .min_content_height(90)
            .child(&body)
            .build();
        root.append(&scroll);
        let expander = gtk::Expander::builder()
            .label("Note options")
            .expanded(false)
            .build();
        expander.add_css_class("sticky-options");
        let fields = vbox(8);
        let tags = gtk::Entry::builder()
            .placeholder_text("Tags, separated by commas")
            .text(note.tags.join(", "))
            .max_length(820)
            .build();
        fields.append(&tags);
        let colors = Color::ALL.iter().map(|c| c.label()).collect::<Vec<_>>();
        let color = gtk::DropDown::from_strings(&colors);
        color.set_selected(
            Color::ALL
                .iter()
                .position(|c| *c == note.color)
                .unwrap_or(0) as u32,
        );
        fields.append(&color);
        let row = hbox(10);
        let pin = gtk::CheckButton::with_label("Pinned");
        pin.set_active(note.pinned);
        let checklist = gtk::CheckButton::with_label("Checklist");
        checklist.set_active(note.checklist);
        row.append(&pin);
        row.append(&checklist);
        fields.append(&row);
        expander.set_child(Some(&fields));
        root.append(&expander);
        let footer = hbox(8);
        let status = text("Saved on this device", "sticky-status");
        status.set_hexpand(true);
        status.set_wrap(true);
        let save = button("Done", "");
        save.set_tooltip_text(Some("Save and close · Ctrl+Enter"));
        footer.append(&status);
        footer.append(&save);
        root.append(&footer);
        let persisted = ui
            .store
            .load_notes()
            .is_ok_and(|notes| notes.iter().any(|n| n.id == note.id));
        let editor = Rc::new(Self {
            window,
            title,
            body,
            tags,
            pin,
            checklist,
            color,
            save,
            board,
            forward,
            note: RefCell::new(note),
            status,
            timer: RefCell::new(None),
            dirty: Cell::new(false),
            syncing: Cell::new(false),
            persisted: Cell::new(persisted),
            ui: ui.clone(),
        });
        {
            let u = ui.clone();
            new.connect_clicked(move |_| {
                u.open_note(None);
            });
        }
        {
            let e = Rc::downgrade(&editor);
            let p = popover.downgrade();
            editor.board.connect_clicked(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if e.flush() {
                    e.ui.view.set(View::All);
                    e.ui.tag.borrow_mut().clear();
                    e.ui.search.set_text("");
                    e.ui.show_board();
                }
            });
        }
        {
            let e = Rc::downgrade(&editor);
            let p = popover.downgrade();
            editor.forward.connect_clicked(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if e.flush() {
                    e.ui.forward_dialog(e.note.borrow().clone());
                }
            });
        }
        {
            let ex = expander.downgrade();
            let p = popover.downgrade();
            options.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(ex) = ex.upgrade() {
                    ex.set_expanded(!ex.is_expanded());
                }
            });
        }
        for entry in [&editor.title, &editor.tags] {
            let e = Rc::downgrade(&editor);
            entry.connect_changed(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                e.queue();
            });
        }
        {
            let e = Rc::downgrade(&editor);
            editor.body.buffer().connect_changed(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                e.queue();
            });
        }
        for check in [&editor.pin, &editor.checklist] {
            let e = Rc::downgrade(&editor);
            check.connect_toggled(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                e.queue();
            });
        }
        {
            let e = Rc::downgrade(&editor);
            editor.color.connect_selected_notify(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                e.queue();
            });
        }
        {
            let e = Rc::downgrade(&editor);
            editor.save.connect_clicked(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                if e.flush() {
                    e.window.close();
                }
            });
        }
        {
            let e = Rc::downgrade(&editor);
            archive.connect_clicked(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                if e.flush() {
                    let note = e.note.borrow().clone();
                    e.ui.move_note(&note, NoteStatus::Archived);
                    e.window.close();
                }
            });
        }
        {
            let e = Rc::downgrade(&editor);
            trash.connect_clicked(move |_| {
                let Some(e) = e.upgrade() else {
                    return;
                };
                if e.flush() {
                    let note = e.note.borrow().clone();
                    e.ui.move_note(&note, NoteStatus::Deleted);
                    e.window.close();
                }
            });
        }
        {
            let e = Rc::downgrade(&editor);
            editor.window.connect_close_request(move |_| {
                let Some(e) = e.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                if !e.flush() {
                    return glib::Propagation::Stop;
                }
                if let Err(error) = e.ui.store.set_setting(
                    &format!("note-size:{}", e.note.borrow().id),
                    &format!("{},{}", e.window.width(), e.window.height()),
                ) {
                    e.status.set_text(&error);
                    return glib::Propagation::Stop;
                }
                e.ui.editors
                    .borrow_mut()
                    .retain(|open| open.note.borrow().id != e.note.borrow().id);
                if !e
                    .ui
                    .app
                    .windows()
                    .iter()
                    .any(|w| w.is_visible() && *w != e.window)
                {
                    e.ui.app.quit();
                }
                glib::Propagation::Proceed
            });
        }
        let controller = gtk::EventControllerKey::new();
        {
            let e = Rc::downgrade(&editor);
            controller.connect_key_pressed(move |_, key, _, mods| {
                let Some(e) = e.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                if key == gdk::Key::Escape {
                    e.window.close();
                    return glib::Propagation::Stop;
                }
                if mods.contains(gdk::ModifierType::CONTROL_MASK) {
                    if key == gdk::Key::Return {
                        e.save.emit_clicked();
                        return glib::Propagation::Stop;
                    }
                    if key == gdk::Key::s {
                        e.flush();
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            });
        }
        editor.window.add_controller(controller);
        editor
    }
    fn replace_note(&self, note: Note) {
        self.syncing.set(true);
        self.title.set_text(&note.title);
        self.body.buffer().set_text(&note.body);
        self.tags.set_text(&note.tags.join(", "));
        self.pin.set_active(note.pinned);
        self.checklist.set_active(note.checklist);
        self.color.set_selected(
            Color::ALL
                .iter()
                .position(|c| *c == note.color)
                .unwrap_or(0) as u32,
        );
        for color in Color::ALL {
            self.window.remove_css_class(color.as_str());
        }
        self.window.add_css_class(note.color.as_str());
        *self.note.borrow_mut() = note;
        self.dirty.set(false);
        self.syncing.set(false);
    }
    fn capture(&self) -> Note {
        let mut note = self.note.borrow().clone();
        note.title = self.title.text().trim().to_string();
        note.body = buffer_text(&self.body);
        note.tags = parse_tags(&self.tags.text());
        note.pinned = self.pin.is_active();
        note.checklist = self.checklist.is_active();
        note.color = Color::ALL
            .get(self.color.selected() as usize)
            .copied()
            .unwrap_or(Color::Paper);
        note.touch();
        note
    }
    fn queue(self: &Rc<Self>) {
        if self.syncing.get() {
            return;
        }
        self.dirty.set(true);
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        self.status.set_text("Saving…");
        let e = self.clone();
        let timer =
            glib::timeout_add_local_once(std::time::Duration::from_millis(350), move || {
                e.timer.borrow_mut().take();
                e.flush();
            });
        *self.timer.borrow_mut() = Some(timer);
    }
    pub fn flush(&self) -> bool {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        if !self.dirty.get() {
            return true;
        }
        let note = self.capture();
        if !self.persisted.get() && note.title.is_empty() && note.body.trim().is_empty() {
            return match self.ui.store.delete_draft(&note.id) {
                Ok(()) => true,
                Err(e) => {
                    self.status.set_text(&e);
                    false
                }
            };
        }
        let result = self
            .ui
            .store
            .save_draft(&note)
            .and_then(|()| self.ui.store.commit_note(&note));
        match result {
            Ok(()) => {
                for c in Color::ALL {
                    self.window.remove_css_class(c.as_str());
                }
                self.window.add_css_class(note.color.as_str());
                self.window.set_title(Some(if note.title.is_empty() {
                    "NERV Fieldnote"
                } else {
                    &note.title
                }));
                *self.note.borrow_mut() = note;
                self.dirty.set(false);
                self.persisted.set(true);
                self.status.set_text("Saved");
                self.ui.refresh();
                true
            }
            Err(e) => {
                self.status.set_text(&format!("Not saved: {e}"));
                false
            }
        }
    }
}

pub fn screenshot(window: &impl IsA<gtk::Widget>, path: &Path) -> Result<(), String> {
    let widget = window.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let node = snapshot.to_node().ok_or("Window has not rendered")?;
    let renderer = widget
        .native()
        .and_then(|n| n.renderer())
        .ok_or("No native renderer")?;
    renderer
        .render_texture(&node, None)
        .save_to_png(path)
        .map_err(|e| e.to_string())
}
