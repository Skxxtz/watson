use crate::{
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{Calendar, Clock, calendar::CalendarEvent},
    },
};
use chrono::NaiveTime;
use common::protocol::Request;
use gtk4::{
    Application, CssProvider,
    gdk::Display,
    gio::{
        ApplicationFlags,
        prelude::{ApplicationExt, ApplicationExtManual},
    },
    glib::subclass::types::ObjectSubclassIsExt,
    prelude::{BoxExt, GtkWindowExt},
};

mod connection;
mod ui;

#[tokio::main]
async fn main() {
    let setup = setup();
    setup.app.connect_activate(|app| {
        // Load css
        let provider = CssProvider::new();
        let display = Display::default().unwrap();
        provider.load_from_resource("/dev/skxxtz/watson/main.css");
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let mut ui = WatsonUi::default();
        let win = ui.window(app);

        win.present();
        let imp = win.imp();

        let clock = Clock::new();
        imp.viewport.append(&clock);

        let events = vec![
            CalendarEvent {
                start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                end: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
                label: "Meeting",
            },
            CalendarEvent {
                start: NaiveTime::from_hms_opt(13, 30, 0).unwrap(),
                end: NaiveTime::from_hms_opt(14, 30, 0).unwrap(),
                label: "Lunch",
            },
            CalendarEvent {
                start: NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
                end: NaiveTime::from_hms_opt(18, 50, 0).unwrap(),
                label: "Call",
            },
            CalendarEvent {
                start: NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
                end: NaiveTime::from_hms_opt(20, 50, 0).unwrap(),
                label: "Example",
            },
            CalendarEvent {
                start: NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
                end: NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
                label: "Another Example",
            },
        ];
        let (area, _calendar) = Calendar::new(events);
        imp.viewport.append(&area);
    });
    setup.app.run();
    connect().await;
}

struct Setup {
    app: Application,
}

fn setup() -> Setup {
    let app = Application::builder()
        .flags(ApplicationFlags::NON_UNIQUE | ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    // Include build resources
    gtk4::gio::resources_register_include!("/resources.gresources")
        .expect("Failed to find resources in OUT_DIR");

    app.connect_command_line(|app, _| {
        app.activate();
        0.into()
    });

    Setup { app }
}

async fn connect() {
    match ClientConnection::new().await {
        Ok(mut c) => {
            if let Ok(response) = c.send(Request::PendingNotifications).await {
                println!("{:?}", response);
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
        }
    }
}
