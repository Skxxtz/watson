use std::{
    cell::RefCell,
    collections::HashMap,
    env,
    rc::Rc,
    sync::{Arc, OnceLock},
};

use crate::{
    config::{WidgetSpec, load_config},
    connection::ClientConnection,
    ui::{
        WatsonUi,
        widgets::{
            BackendFunc, Battery, Button, NotificationCentre, Slider, WatsonWidget, create_widgets,
        },
    },
};
use common::{
    config::flags::ArgParse,
    notification::Notification,
    protocol::{AtomicSystemState, Request, Response, UpdateField},
    utils::errors::WatsonError,
};
use gtk4::{
    CssProvider, DrawingArea,
    gdk::Display,
    glib::{WeakRef, object::ObjectExt, subclass::types::ObjectSubclassIsExt},
    prelude::GtkWindowExt,
};
use tokio::sync::{Notify, broadcast, mpsc::UnboundedSender};

mod config;
mod connection;
mod ui;

static DAEMON_TX: OnceLock<UnboundedSender<Request>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), WatsonError> {
    gtk4::init().expect("Failed to init GTK");
    let main_loop = gtk4::glib::MainLoop::new(None, false);

    let (tx, rx) = broadcast::channel::<Response>(64);

    let _ = ArgParse::parse(std::env::args()).await;
    let state = Rc::new(RefCell::new(WatsonState::new()));

    let notify = Arc::new(Notify::new());

    let client = ClientConnection::new().await?;
    DAEMON_TX
        .set(
            client
                .spawn_engine(
                    tx.clone(),
                    Arc::clone(&state.borrow().system_state),
                    Arc::clone(&notify),
                )
                .await?,
        )
        .expect("DAEMON_TX already set");

    let notification_store = Rc::new(RefCell::new(NotificationStore::new()));

    gtk4::gio::resources_register_include!("/resources.gresources")
        .expect("Failed to find resources in OUT_DIR");

    let config = load_config()?;
    let required_services = config
        .iter()
        .map(WidgetSpec::required_services)
        .reduce(|a, b| a | b)
        .unwrap_or(0);

    // Load css
    let provider = CssProvider::new();
    let display = Display::default().unwrap();
    provider.load_from_resource("/dev/skxxtz/watson/main.css");
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Listen async for server responses/notifications
    let ui_ready = Rc::new(Notify::new());
    gtk4::glib::spawn_future_local({
        let mut rx = rx.resubscribe();
        let state = Rc::clone(&state);
        let store = Rc::clone(&notification_store);
        let ui_ready = Rc::clone(&ui_ready);
        let notify = Arc::clone(&notify);
        async move {
            loop {
                tokio::select! {
                    biased;
                    _ = notify.notified() => {
                        let mask = state.borrow().system_state.updated.swap(0, std::sync::atomic::Ordering::Relaxed);
                        if mask & (1 << UpdateField::Init as u8) != 0 {
                            ui_ready.notify_one();
                        }
                        if mask & (1 << UpdateField::Wifi as u8) != 0 {
                            state.borrow().button(BackendFunc::Wifi).for_each(|c| c.queue_draw());
                        }

                        if mask & (1 << UpdateField::Bluetooth as u8) != 0 {
                            state.borrow().button(BackendFunc::Bluetooth).for_each(|c| c.queue_draw());
                        }

                        if mask & (1 << UpdateField::Powermode as u8) != 0 {
                            state.borrow().button(BackendFunc::Powermode).for_each(|c| c.queue_draw());
                        }

                        if mask & (1 << UpdateField::Brightness as u8) != 0 {
                            state.borrow().slider(BackendFunc::Brightness).for_each(|c| c.queue_draw());
                        }

                        if mask & (1 << UpdateField::Volume as u8) != 0 {
                            state.borrow().slider(BackendFunc::Volume).for_each(|c| c.queue_draw());
                        }
                    }
                    Ok(msg) = rx.recv() => {
                        match msg {
                            Response::BatteryState {
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
                            _ => {
                                println!("{:?}", msg);
                            }
                        }
                    },
                }
            }
        }
    });

    // Make initial requests
    if let Some(daemon) = DAEMON_TX.get() {
        let _result = daemon.send(Request::RegisterServices(required_services));
    }

    let mut ui = WatsonUi::default();
    let win = ui.window();

    win.connect_close_request({
        let main_loop = main_loop.clone();
        move |_| {
            main_loop.quit();
            gtk4::glib::Propagation::Stop
        }
    });

    gtk4::glib::spawn_future_local({
        let state = Rc::clone(&state);
        let win = win.downgrade();
        async move {
            ui_ready.notified().await;
            // async wait for a notify signal
            if let Some(win) = win.upgrade() {
                let imp = win.imp();
                for spec in config {
                    create_widgets(&imp.viewport.get(), spec, Rc::clone(&state), false);
                }
                win.present();
            }
        }
    });

    main_loop.run();

    Ok(())
}

#[derive(Default)]
pub struct WatsonState {
    system_state: Arc<AtomicSystemState>,

    widgets: Vec<WatsonWidget>,
    buttons: HashMap<BackendFunc, Vec<Button>>,
    sliders: HashMap<BackendFunc, Vec<Slider>>,
}
#[allow(dead_code)]
impl WatsonState {
    pub fn new() -> Self {
        Self {
            system_state: Arc::new(AtomicSystemState::default()),

            widgets: Vec::new(),
            buttons: HashMap::new(),
            sliders: HashMap::new(),
        }
    }
    pub fn register_widge(&mut self, widget: WatsonWidget) {
        match widget {
            WatsonWidget::Button(b) => {
                self.buttons.entry(b.func).or_default().push(b);
            }
            WatsonWidget::Slider(s) => {
                self.sliders.entry(s.func).or_default().push(s);
            }
            _ => {}
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
                Some(&c.area)
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
    pub fn button(&self, func: BackendFunc) -> impl Iterator<Item = &Button> {
        self.buttons.get(&func).into_iter().flat_map(|v| v.iter())
    }
    pub fn slider(&self, func: BackendFunc) -> impl Iterator<Item = &Slider> {
        self.sliders.get(&func).into_iter().flat_map(|v| v.iter())
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
