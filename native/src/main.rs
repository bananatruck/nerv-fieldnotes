mod cli;
mod model;
mod smoke;
mod store;
mod ui;

use gtk::{gio, glib, prelude::*};
use std::{cell::RefCell, path::Path, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Launch {
    Recent,
    Board,
    NewNote,
    Smoke,
}

impl Launch {
    fn from_flags(board: bool, new_note: bool, smoke: bool) -> Result<Self, String> {
        if [board, new_note, smoke]
            .into_iter()
            .filter(|flag| *flag)
            .count()
            > 1
        {
            return Err("Choose only one of --board, --new-note, or --smoke-test.".into());
        }
        Ok(if smoke {
            Self::Smoke
        } else if board {
            Self::Board
        } else if new_note {
            Self::NewNote
        } else {
            Self::Recent
        })
    }
}

type CurrentUi = Rc<RefCell<Option<Rc<ui::Ui>>>>;

fn get_ui(app: &gtk::Application, current: &CurrentUi, path: &Path) -> Result<Rc<ui::Ui>, String> {
    if let Some(ui) = current.borrow().clone() {
        return Ok(ui);
    }
    let ui = ui::Ui::new(app, store::Store::open(path)?);
    *current.borrow_mut() = Some(ui.clone());
    Ok(ui)
}

fn open(
    app: &gtk::Application,
    current: &CurrentUi,
    path: &Path,
    launch: Launch,
) -> Result<(), String> {
    let ui = get_ui(app, current, path)?;
    match launch {
        Launch::Recent => ui.launch_recent()?,
        Launch::Board => ui.show_board(),
        Launch::NewNote => {
            ui.open_note(None);
        }
        Launch::Smoke => smoke::schedule(ui),
    }
    Ok(())
}

fn show_startup_error(app: &gtk::Application, message: &str) {
    // An application window keeps GTK alive so launch failures remain visible.
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Fieldnotes could not open its data")
        .default_width(520)
        .default_height(240)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let heading = gtk::Label::new(Some("Fieldnotes could not open its data"));
    heading.add_css_class("title-2");
    heading.set_wrap(true);
    heading.set_xalign(0.0);
    content.append(&heading);
    let detail = gtk::Label::new(Some(&format!(
        "{message}\n\nYour database was not replaced."
    )));
    detail.set_wrap(true);
    detail.set_selectable(true);
    detail.set_xalign(0.0);
    detail.set_vexpand(true);
    content.append(&detail);
    let close = gtk::Button::with_label("Close");
    close.set_halign(gtk::Align::End);
    let weak_window = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak_window.upgrade() {
            window.close();
        }
    });
    content.append(&close);
    window.set_child(Some(&content));
    window.present();
}

fn add_gui_options(app: &gtk::Application) {
    for (name, description) in [
        ("board", "Open the complete note board"),
        ("new-note", "Open a new sticky note"),
    ] {
        app.add_main_option(
            name,
            glib::Char::from(0u8),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            description,
            None,
        );
    }
    app.add_main_option(
        "data-dir",
        glib::Char::from(0u8),
        glib::OptionFlags::NONE,
        glib::OptionArg::Filename,
        "Use an isolated notes and inbox directory",
        Some("DIR"),
    );
    app.add_main_option(
        "smoke-test",
        glib::Char::from(0u8),
        glib::OptionFlags::HIDDEN,
        glib::OptionArg::None,
        "Run native GTK checks in an isolated instance",
        None,
    );
}

fn main() -> glib::ExitCode {
    let argv = std::env::args().collect::<Vec<_>>();
    let args = &argv[1..];
    match cli::run(args) {
        Ok(true) => return glib::ExitCode::SUCCESS,
        Ok(false) => {}
        Err(error) => {
            eprintln!("{error}");
            return glib::ExitCode::FAILURE;
        }
    }
    if let Err(error) = Launch::from_flags(
        args.iter().any(|arg| arg == "--board"),
        args.iter().any(|arg| arg == "--new-note"),
        args.iter().any(|arg| arg == "--smoke-test"),
    ) {
        eprintln!("{error}");
        return glib::ExitCode::FAILURE;
    }
    let path = match cli::database_path(args) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return glib::ExitCode::FAILURE;
        }
    };
    let test = args.iter().any(|arg| arg == "--smoke-test");
    if test
        && !args
            .iter()
            .any(|arg| arg == "--data-dir" || arg.starts_with("--data-dir="))
    {
        eprintln!("--smoke-test requires --data-dir DIR to protect your notes");
        return glib::ExitCode::FAILURE;
    }
    let isolated = test
        || args
            .iter()
            .any(|arg| arg == "--data-dir" || arg.starts_with("--data-dir="));
    let mut flags = gio::ApplicationFlags::HANDLES_COMMAND_LINE;
    if isolated {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = gtk::Application::builder()
        .application_id(if test {
            "dev.kesh.Fieldnotes.Test"
        } else {
            "dev.kesh.Fieldnotes"
        })
        .flags(flags)
        .build();
    add_gui_options(&app);
    let current: CurrentUi = Rc::new(RefCell::new(None));
    {
        let current = current.clone();
        let path = path.clone();
        app.connect_activate(move |app| {
            if let Err(error) = open(app, &current, &path, Launch::Recent) {
                eprintln!("Cannot open Fieldnotes: {error}");
                show_startup_error(app, &error);
            }
        });
    }
    app.connect_command_line(move |app, command_line| {
        let options = command_line.options_dict();
        let launch = Launch::from_flags(
            options.contains("board"),
            options.contains("new-note"),
            options.contains("smoke-test"),
        );
        // Registered GUI options are removed from the forwarded argument vector.
        if command_line.arguments().len() > 1 {
            eprintln!("Unexpected GUI argument. Run nerv-fieldnotes --help for usage.");
            return glib::ExitCode::FAILURE;
        }
        let launch = match launch {
            Ok(launch) => launch,
            Err(error) => {
                eprintln!("{error}");
                return glib::ExitCode::FAILURE;
            }
        };
        match open(app, &current, &path, launch) {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Cannot open Fieldnotes: {error}");
                if test {
                    smoke::FAILED.store(true, std::sync::atomic::Ordering::SeqCst);
                    app.quit();
                } else {
                    show_startup_error(app, &error);
                }
                glib::ExitCode::FAILURE
            }
        }
    });
    let result = app.run_with_args(&argv);
    if test && smoke::FAILED.load(std::sync::atomic::Ordering::SeqCst) {
        glib::ExitCode::FAILURE
    } else {
        result
    }
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    #[test]
    fn default_startup_opens_only_the_recent_sticky() {
        assert_eq!(
            Launch::from_flags(false, false, false).unwrap(),
            Launch::Recent
        );
        assert_eq!(
            Launch::from_flags(true, false, false).unwrap(),
            Launch::Board
        );
        assert_eq!(
            Launch::from_flags(false, true, false).unwrap(),
            Launch::NewNote
        );
        assert_eq!(
            Launch::from_flags(false, false, true).unwrap(),
            Launch::Smoke
        );
    }

    #[test]
    fn conflicting_gui_modes_are_rejected() {
        assert!(Launch::from_flags(true, true, false).is_err());
        assert!(Launch::from_flags(true, false, true).is_err());
    }
}
