use std::env;

use crate::{
    config::{WidgetSpec, load_config},
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{Battery, Calendar, Clock},
    },
};
use common::{config::flags::ArgParse, protocol::Request};
use gtk4::{
    Application, Box, CssProvider,
    gdk::Display,
    gio::{
        ApplicationFlags,
        prelude::{ApplicationExt, ApplicationExtManual},
    },
    glib::subclass::types::ObjectSubclassIsExt,
    prelude::{BoxExt, GtkWindowExt, WidgetExt},
};

mod config;
mod connection;
mod ui;

#[tokio::main]
async fn main() {
    let x = ArgParse::parse(std::env::args());
    println!("{:?}", x);
    let setup = setup();
    setup.app.connect_activate(|app| {
        // Load Config
        let config = load_config().unwrap_or_default();

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
        let imp = win.imp();
        win.present();

        for spec in config {
            create_widgets(&imp.viewport.get(), spec);
        }
    });
    setup.app.run();
    connect().await;
}

fn create_widgets(viewport: &Box, spec: WidgetSpec) {
    match spec {
        WidgetSpec::Battery { .. } => {
            let bat = Battery::new(&spec);
            viewport.append(&bat);
        }
        WidgetSpec::Calendar { .. } => {
            let calendar = Calendar::new(&spec);
            viewport.append(&calendar);
        }
        WidgetSpec::Clock { .. } => {
            let clock = Clock::new(&spec);
            viewport.append(&clock);
        }
        WidgetSpec::Column {
            base,
            spacing,
            children,
        } => {
            let col = Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .valign(gtk4::Align::Start)
                .halign(gtk4::Align::Start)
                .spacing(spacing)
                .build();

            if let Some(id) = base.id {
                col.set_widget_name(&id);
            }

            viewport.append(&col);

            for child in children {
                create_widgets(&col, child);
            }
        }
        WidgetSpec::Row {
            base,
            spacing,
            children,
        } => {
            let row = Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .valign(gtk4::Align::Start)
                .halign(gtk4::Align::Start)
                .spacing(spacing)
                .build();
            if let Some(id) = base.id {
                row.set_widget_name(&id);
            }

            viewport.append(&row);

            for child in children {
                create_widgets(&row, child);
            }
        }
    }
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
