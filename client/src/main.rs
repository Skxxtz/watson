use std::{cell::RefCell, rc::Rc};

use crate::{
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{Calendar, Clock},
    },
};
use chrono::Local;
use common::calendar::icloud::PropfindInterface;
use common::protocol::Request;
use gtk4::{
    Application, CssProvider,
    gdk::Display,
    gio::{
        ApplicationFlags,
        prelude::{ApplicationExt, ApplicationExtManual},
    },
    glib::{object::ObjectExt, subclass::types::ObjectSubclassIsExt},
    prelude::{BoxExt, GtkWindowExt, WidgetExt},
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

        let events = Rc::new(RefCell::new(Vec::new()));

        let (area, _calendar) = Calendar::new(Rc::clone(&events));
        imp.viewport.append(&area);

        gtk4::glib::MainContext::default().spawn_local({
            let events = Rc::clone(&events);
            let cal_weak = area.downgrade();
            async move {
                let mut interface = PropfindInterface::new();

                match interface.get_principal().await {
                    Ok(_) => match interface.get_calendars().await {
                        Ok(calendar_info) => match interface.get_events(calendar_info).await {
                            Ok(mut evs) => {
                                let today = Local::now().to_utc();
                                evs.retain(|e| e.occurs_on_day(&today));
                                events.borrow_mut().extend(evs);
                                if let Some(clock) = cal_weak.upgrade() {
                                    clock.queue_draw();
                                }
                            }
                            Err(e) => eprintln!("Failed to fetch events: {:?}", e),
                        },
                        Err(e) => eprintln!("Failed to fetch calendars: {:?}", e),
                    },
                    Err(e) => eprintln!("Failed to get principal: {:?}", e),
                }
            }
        });
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
