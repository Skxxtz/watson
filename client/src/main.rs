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
        widgets::{BackendFunc, Battery, NotificationCentre, WatsonWidget, create_widgets},
    },
};
use common::{
    config::flags::ArgParse,
    notification::Notification,
    protocol::{AtomicSystemState, Request, Response, UpdateField},
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};
use gtk4::{
    CssProvider, DrawingArea,
    gdk::Display,
    glib::{WeakRef, object::ObjectExt, subclass::types::ObjectSubclassIsExt},
    prelude::{GtkWindowExt, WidgetExt},
};
use tokio::{
    sync::{Notify, broadcast, mpsc::UnboundedSender},
    task::spawn_blocking,
};

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
        .expect("Failed to find resources injo OUT_DIR");

    let config = spawn_blocking(|| load_config());

    // Load css
    gtk4::glib::idle_add_full(gtk4::glib::Priority::HIGH_IDLE, move || {
        let provider = CssProvider::new();
        let display = Display::default().unwrap();
        provider.load_from_resource("/dev/skxxtz/watson/main.css");

        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Return ControlFlow::Break so it only runs once
        gtk4::glib::ControlFlow::Break
    });

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
                        let state_ref = state.borrow();
                        let mask = state_ref.system_state.updated.swap(0, std::sync::atomic::Ordering::Relaxed);
                        if mask & (1 << UpdateField::Init as u8) != 0 {
                            ui_ready.notify_one();
                        }
                        if mask & (1 << UpdateField::Wifi as u8) != 0 {
                            state_ref.notify_update(BackendFunc::Wifi);
                        }

                        if mask & (1 << UpdateField::Bluetooth as u8) != 0 {
                            state_ref.notify_update(BackendFunc::Bluetooth);
                        }

                        if mask & (1 << UpdateField::Powermode as u8) != 0 {
                            state_ref.notify_update(BackendFunc::Powermode);
                        }

                        if mask & (1 << UpdateField::Brightness as u8) != 0 {
                            state_ref.notify_update(BackendFunc::Brightness);
                        }

                        if mask & (1 << UpdateField::Volume as u8) != 0 {
                            state_ref.notify_update(BackendFunc::Volume);
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
    let config = config
        .await
        .map_err(|e| watson_err!(WatsonErrorKind::TaskJoin, e.to_string()))??;

    let required_services = config
        .iter()
        .map(WidgetSpec::required_services)
        .reduce(|a, b| a | b)
        .unwrap_or(0);

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

    win.present();
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
    subscribers: HashMap<BackendFunc, Vec<WeakRef<gtk4::Widget>>>,
}
#[allow(dead_code)]
impl WatsonState {
    pub fn new() -> Self {
        Self {
            system_state: Arc::new(AtomicSystemState::default()),

            widgets: Vec::new(),
            subscribers: HashMap::new(),
        }
    }
    pub fn register_widget(&mut self, widget: WatsonWidget) {
        match widget {
            WatsonWidget::Button(b) => {
                self.subscribers
                    .entry(b.func.func())
                    .or_default()
                    .push(b.weak);
            }
            WatsonWidget::Slider(s) => {
                self.subscribers
                    .entry(s.func.func())
                    .or_default()
                    .push(s.weak);
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
    pub fn notify_update(&self, func: BackendFunc) {
        if let Some(subs) = self.subscribers.get(&func) {
            subs.iter()
                .filter_map(|w| w.upgrade())
                .for_each(|widget| widget.queue_draw());
        }
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
