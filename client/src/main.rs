use std::{cell::RefCell, env, rc::Rc};

use crate::{
    config::{WidgetSpec, load_config},
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{
            Battery, BatteryBuilder, Button, ButtonBuilder, ButtonFunc, Calendar, Clock,
            NotificationCentre, NotificationCentreBuilder, Slider, SliderBuilder,
        },
    },
};
use common::{
    config::flags::ArgParse, notification::Notification, protocol::Response,
    tokio::AsyncSizedMessage,
};
use gtk4::{
    Application, Box, CssProvider, DrawingArea, Separator,
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

    let _ = ArgParse::parse(std::env::args()).await;
    let _ = connect(tx).await;

    let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

    let setup = setup();
    setup.app.connect_activate({
        let state = Rc::clone(&state);
        move |app| {
            // Load Config
            let config = match load_config() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{:?}", e);
                    return;
                }
            };

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
                create_widgets(&imp.viewport.get(), spec, Rc::clone(&state), false);
            }

            // Listen async for server responses/notifications
            gtk4::glib::spawn_future_local({
                let mut rx = rx.resubscribe();
                let state = Rc::clone(&state);
                let store = Rc::clone(&notification_store);
                async move {
                    while let Ok(buf) = rx.recv().await {
                        match serde_json::from_slice::<Response>(&buf) {
                            Ok(b) => match b {
                                Response::BatteryStateChange {
                                    state: s,
                                    percentage: p,
                                } => {
                                    state.borrow().batteries().for_each(|bat| {
                                        bat.update_state(s, p);
                                        bat.queue_draw();
                                    });
                                }
                                Response::Notification(Some(notification)) => {
                                    let rc = Rc::new(notification);
                                    state.borrow().notification_centres().for_each(|c| {
                                        c.insert(rc.clone());
                                    });
                                    store.borrow_mut().notifications.push(rc);
                                }
                                Response::Notifications(s) => {
                                    store
                                        .borrow_mut()
                                        .notifications
                                        .extend(s.into_iter().map(|v| Rc::new(v)));
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

fn create_widgets(viewport: &Box, spec: WidgetSpec, state: Rc<RefCell<UiState>>, in_holder: bool) {
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
                .push(WatsonWidget::Calendar(ObjectExt::downgrade(&calendar)));

            viewport.append(&calendar);
        }
        WidgetSpec::Clock { .. } => {
            let clock = Clock::new(&spec);

            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Clock(ObjectExt::downgrade(&clock)));

            viewport.append(&clock);
        }
        WidgetSpec::Notifications { .. } => {
            let notification_centre = NotificationCentreBuilder::new().for_box(&viewport).build();

            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::NoticationCentre(notification_centre));
        }
        WidgetSpec::Button { .. } => {
            let button = ButtonBuilder::new(&spec, in_holder)
                .for_box(&viewport)
                .build();
            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Button(button));
        }
        WidgetSpec::Slider { .. } => {
            let slider = SliderBuilder::new(&spec, in_holder)
                .for_box(&viewport)
                .build();
            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Slider(slider));
        }
        WidgetSpec::Column {
            base,
            spacing,
            children,
        } => {
            let col = Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .valign(gtk4::Align::Fill)
                .halign(gtk4::Align::Fill)
                .vexpand(true)
                .hexpand(true)
                .spacing(spacing)
                .build();

            if let Some(id) = base.id {
                col.set_widget_name(&id);
            }

            viewport.append(&col);

            for child in children {
                create_widgets(&col, child, state.clone(), true);
            }
        }
        WidgetSpec::Row {
            base,
            spacing,
            children,
        } => {
            let row = Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .valign(gtk4::Align::Fill)
                .halign(gtk4::Align::Fill)
                .hexpand(true)
                .vexpand(true)
                .spacing(spacing)
                .build();
            if let Some(id) = base.id {
                row.set_widget_name(&id);
            }

            viewport.append(&row);

            for child in children {
                create_widgets(&row, child, state.clone(), true);
            }
        }
        WidgetSpec::Spacer { base } => {
            let spacer = Box::builder()
                .css_classes(["widget", "spacer"])
                .valign(gtk4::Align::Fill)
                .halign(gtk4::Align::Fill)
                .hexpand(true)
                .vexpand(true)
                .height_request(10)
                .width_request(10)
                .build();

            if let Some(id) = base.id {
                spacer.set_widget_name(&id);
            }

            viewport.append(&spacer);
        }
        WidgetSpec::Separator { base } => {
            let separator = Separator::builder()
                .css_classes(["separator"])
                .valign(gtk4::Align::Fill)
                .halign(gtk4::Align::Fill)
                .hexpand(true)
                .vexpand(true)
                .build();

            if let Some(id) = base.id {
                separator.set_widget_name(&id);
            }

            viewport.append(&separator);
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
    NoticationCentre(NotificationCentre),
    Button(Button),
    Slider(Slider),
}
impl WatsonWidget {
    pub fn widget_type(&self) -> WatsonWidgetType {
        match self {
            Self::Battery(_) => WatsonWidgetType::Battery,
            Self::Calendar(_) => WatsonWidgetType::Calendar,
            Self::Clock(_) => WatsonWidgetType::Clock,
            Self::NoticationCentre(_) => WatsonWidgetType::NotificationCentre,
            Self::Button(_) => WatsonWidgetType::Button,
            Self::Slider(_) => WatsonWidgetType::Slider,
        }
    }
}
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum WatsonWidgetType {
    Battery,
    Calendar,
    Clock,
    NotificationCentre,
    Button,
    Slider,
}

#[derive(Default)]
pub struct UiState {
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
    pub fn notification_centres(&self) -> impl Iterator<Item = &NotificationCentre> {
        self.widgets.iter().filter_map(|w| {
            if let WatsonWidget::NoticationCentre(c) = w {
                Some(c)
            } else {
                None
            }
        })
    }
    pub fn button(&self, func: ButtonFunc) -> impl Iterator<Item = &Button> {
        self.widgets
            .iter()
            .filter_map(|w| {
                if let WatsonWidget::Button(b) = w {
                    Some(b)
                } else {
                    None
                }
            })
            .filter(move |b| b.func == func)
    }
}

struct NotificationStore {
    notifications: Vec<Rc<Notification>>,
}
impl NotificationStore {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
        }
    }
}
