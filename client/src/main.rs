use std::{cell::RefCell, env, rc::Rc, sync::OnceLock};

use crate::{
    config::{WidgetSpec, load_config},
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{
            Battery, BatteryBuilder, Button, ButtonBuilder, ButtonFunc, Calendar, Clock,
            NotificationCentre, NotificationCentreBuilder, Slider, SliderBuilder, SliderFunc,
        },
    },
};
use common::{
    config::flags::ArgParse,
    errors::WatsonError,
    notification::Notification,
    protocol::{Request, Response, SystemState},
};
use futures::executor::block_on;
use gtk4::{
    Align, Application, AspectFrame, Box, CssProvider, DrawingArea, Separator,
    gdk::Display,
    gio::{
        ApplicationFlags,
        prelude::{ApplicationExt, ApplicationExtManual},
    },
    glib::{WeakRef, object::ObjectExt, subclass::types::ObjectSubclassIsExt},
    prelude::{BoxExt, GtkWindowExt, WidgetExt},
};
use tokio::sync::{broadcast, mpsc::UnboundedSender};

mod config;
mod connection;
mod ui;

static DAEMON_TX: OnceLock<UnboundedSender<Request>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), WatsonError> {
    let (tx, rx) = broadcast::channel::<Vec<u8>>(2);
    let state = Rc::new(RefCell::new(WatsonState::new()));

    let _ = ArgParse::parse(std::env::args()).await;
    let client = ClientConnection::new().await?;
    DAEMON_TX
        .set(client.spawn_engine(tx).await)
        .expect("DAEMON_TX already set");

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

            // Register services with daemon
            let required_services = config
                .iter()
                .map(WidgetSpec::required_services)
                .reduce(|a, b| a | b)
                .unwrap_or(0);
            DAEMON_TX
                .get()
                .map(|d| d.send(Request::RegisterServices(required_services)));

            // Send State Query to Daemon
            DAEMON_TX.get().map(|d| d.send(Request::SystemState));
            let system_state = block_on({
                let mut rx = rx.resubscribe();
                async move {
                    if let Ok(msg) = rx.recv().await {
                        if let Ok(resp) = bincode::deserialize::<Response>(&msg) {
                            if let Response::SystemState(s) = resp {
                                return Some(s);
                            }
                        }
                    }
                    None
                }
            });

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

            let system_state = Rc::new(system_state.unwrap_or_default());
            for spec in config {
                create_widgets(
                    &imp.viewport.get(),
                    spec,
                    Rc::clone(&state),
                    Rc::clone(&system_state),
                    false,
                );
            }

            // Listen async for server responses/notifications
            gtk4::glib::spawn_future_local({
                let mut rx = rx.resubscribe();
                let state = Rc::clone(&state);
                let store = Rc::clone(&notification_store);
                async move {
                    while let Ok(buf) = rx.recv().await {
                        match bincode::deserialize::<Response>(&buf) {
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
                                Response::SystemState(s) => {
                                    state.borrow_mut().system_state.replace(s);
                                }
                                _ => {
                                    println!("{:?}", b);
                                }
                            },
                            Err(e) => {
                                eprintln!("{e}: {}", String::from_utf8_lossy(&buf))
                            }
                        }
                    }
                }
            });
        }
    });
    setup.app.run();
    Ok(())
}

fn create_widgets(
    viewport: &Box,
    spec: WidgetSpec,
    state: Rc<RefCell<WatsonState>>,
    system_state: Rc<SystemState>,
    in_holder: bool,
) {
    match spec {
        WidgetSpec::Battery { .. } => {
            let bat = BatteryBuilder::new(&spec, in_holder)
                .for_box(&viewport)
                .build();

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
            let notification_centre = NotificationCentreBuilder::new(&spec)
                .for_box(&viewport)
                .build();

            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::NotificationCentre(notification_centre));
        }
        WidgetSpec::Button { .. } => {
            let button = ButtonBuilder::new(&spec, Rc::clone(&system_state), in_holder)
                .for_box(&viewport)
                .build();
            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Button(button));
        }
        WidgetSpec::Slider { .. } => {
            let slider = SliderBuilder::new(&spec, Rc::clone(&system_state), in_holder)
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
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
                .vexpand(true)
                .hexpand(true)
                .spacing(spacing)
                .build();

            if let Some(id) = base.id {
                col.set_widget_name(&id);
            }

            if let Some(ratio) = base.ratio {
                let aspect_frame = AspectFrame::builder()
                    .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                    .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
                    .ratio(ratio)
                    .obey_child(false)
                    .child(&col)
                    .build();
                viewport.append(&aspect_frame);
            } else {
                viewport.append(&col);
            }

            for child in children {
                create_widgets(&col, child, state.clone(), Rc::clone(&system_state), true);
            }
        }
        WidgetSpec::Row {
            base,
            spacing,
            children,
        } => {
            let row = Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
                .hexpand(true)
                .vexpand(true)
                .spacing(spacing)
                .build();
            if let Some(id) = base.id {
                row.set_widget_name(&id);
            }

            if let Some(ratio) = base.ratio {
                let aspect_frame = AspectFrame::builder()
                    .ratio(ratio)
                    .obey_child(false)
                    .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                    .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
                    .child(&row)
                    .build();
                viewport.append(&aspect_frame);
            } else {
                viewport.append(&row);
            }

            for child in children {
                create_widgets(&row, child, state.clone(), Rc::clone(&system_state), true);
            }
        }
        WidgetSpec::Spacer { base } => {
            let spacer = Box::builder()
                .css_classes(["widget", "spacer"])
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
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
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
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

macro_rules! define_widgets {
    ($($name:ident($data:ty)),* $(,)?) => {
        #[derive(PartialEq, Copy, Clone, Debug)]
        pub enum WatsonWidgetType {
            $($name),*
        }

        #[derive(Debug, Clone)]
        pub enum WatsonWidget {
            $($name($data)),*
        }

        impl WatsonWidget {
            pub fn widget_type(&self) -> WatsonWidgetType {
                match self {
                    $(Self::$name(_) => WatsonWidgetType::$name),*
                }
            }

            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$name(_) => stringify!($name)),*
                }
            }
        }
    };
}
define_widgets! {
    Battery(Battery),
    Calendar(WeakRef<DrawingArea>),
    Clock(WeakRef<DrawingArea>),
    NotificationCentre(NotificationCentre),
    Button(Button),
    Slider(Slider),
}

#[derive(Default)]
pub struct WatsonState {
    widgets: Vec<WatsonWidget>,
    system_state: Option<SystemState>,
}
#[allow(dead_code)]
impl WatsonState {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            system_state: None,
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
            if let WatsonWidget::NotificationCentre(c) = w {
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
    pub fn slider(&self, func: SliderFunc) -> impl Iterator<Item = &Slider> {
        self.widgets
            .iter()
            .filter_map(|w| {
                if let WatsonWidget::Slider(s) = w {
                    Some(s)
                } else {
                    None
                }
            })
            .filter(move |s| s.func == func)
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
