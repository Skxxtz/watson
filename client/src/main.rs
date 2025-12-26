use std::{cell::RefCell, env, rc::Rc};

use crate::{
    config::{WidgetSpec, load_config},
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{Battery, BatteryBuilder, Calendar, Clock, NotificationCentreBuilder},
    },
};
use common::{config::flags::ArgParse, protocol::Response, tokio::AsyncSizedMessage};
use gtk4::{
    Application, Box, CssProvider, DrawingArea,
    gdk::Display,
    gio::{
        ApplicationFlags,
        prelude::{ApplicationExt, ApplicationExtManual},
    },
    glib::{WeakRef, object::ObjectExt, subclass::types::ObjectSubclassIsExt},
    prelude::{BoxExt, GtkWindowExt, WidgetExt},
};
use tokio::sync::broadcast;

mod config;
mod connection;
mod ui;

#[tokio::main]
async fn main() {
    let (tx, rx) = broadcast::channel::<Vec<u8>>(2);
    let state = Rc::new(RefCell::new(UiState::new()));

    let _ = ArgParse::parse(std::env::args());
    let _ = connect(tx).await;

    let setup = setup();
    setup.app.connect_activate({
        let state = Rc::clone(&state);
        move |app| {
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
                create_widgets(&imp.viewport.get(), spec, Rc::clone(&state));
            }

            // Listen async for server responses/notifications
            gtk4::glib::spawn_future_local({
                let mut rx = rx.resubscribe();
                let state = Rc::clone(&state);
                async move {
                    while let Ok(buf) = rx.recv().await {
                        match serde_json::from_slice::<Response>(&buf) {
                            Ok(b) => match b {
                                Response::BatteryStateChange(s) => {
                                    state.borrow().batteries().for_each(|bat| {
                                        bat.update_state(s);
                                        bat.queue_draw();
                                    });
                                }
                                _ => {}
                            },
                            Err(_) => {}
                        }
                    }
                }
            });
        }
    });
    setup.app.run();
}

fn create_widgets(viewport: &Box, spec: WidgetSpec, state: Rc<RefCell<UiState>>) {
    match spec {
        WidgetSpec::Battery { .. } => {
            let bat = BatteryBuilder::new(&spec).for_box(&viewport).build();

            state.borrow_mut().widgets.push(WatsonWidget::Battery(bat));
        }
        WidgetSpec::Calendar { .. } => {
            let calendar = Calendar::new(&spec);

            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Calendar(calendar.downgrade()));

            viewport.append(&calendar);
        }
        WidgetSpec::Clock { .. } => {
            let clock = Clock::new(&spec);

            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Clock(clock.downgrade()));

            viewport.append(&clock);
        }
        WidgetSpec::Notifications { .. } => {
            NotificationCentreBuilder::new().for_box(&viewport);
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
                create_widgets(&col, child, state.clone());
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
                create_widgets(&row, child, state.clone());
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

async fn connect(sender: broadcast::Sender<Vec<u8>>) {
    match ClientConnection::new().await {
        Ok(c) => {
            let mut read_stream = c.stream;
            tokio::spawn(async move {
                loop {
                    match read_stream.read_sized().await {
                        Ok(buf) => {
                            if buf.is_empty() {
                                continue;
                            }
                            let _ = sender.send(buf);
                        }
                        Err(e) => {
                            eprintln!("Connection closed: {e}");
                            break;
                        }
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("{e}");
        }
    };
}

#[derive(Clone, Debug)]
pub enum WatsonWidget {
    Battery(Battery),
    Calendar(WeakRef<DrawingArea>),
    Clock(WeakRef<DrawingArea>),
}
impl WatsonWidget {
    pub fn widget_type(&self) -> WatsonWidgetType {
        match self {
            Self::Battery(_) => WatsonWidgetType::Battery,
            Self::Calendar(_) => WatsonWidgetType::Calendar,
            Self::Clock(_) => WatsonWidgetType::Clock,
        }
    }
}
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum WatsonWidgetType {
    Battery,
    Calendar,
    Clock,
}

struct UiState {
    widgets: Vec<WatsonWidget>,
}
#[allow(dead_code)]
impl UiState {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
        }
    }
    pub fn batteries(&self) -> impl Iterator<Item = &Battery> {
        self.widgets.iter().filter_map(|w| {
            if let WatsonWidget::Battery(c) = w {
                Some(c)
            } else {
                None
            }
        })
    }
    pub fn calendars(&self) -> impl Iterator<Item = &WeakRef<DrawingArea>> {
        self.widgets.iter().filter_map(|w| {
            if let WatsonWidget::Calendar(c) = w {
                Some(c)
            } else {
                None
            }
        })
    }
    pub fn clocks(&self) -> impl Iterator<Item = &WeakRef<DrawingArea>> {
        self.widgets.iter().filter_map(|w| {
            if let WatsonWidget::Clock(c) = w {
                Some(c)
            } else {
                None
            }
        })
    }
}
