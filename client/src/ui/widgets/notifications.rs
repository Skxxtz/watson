use std::rc::Rc;

use common::notification::Notification;
use gtk4::{
    Box, ListBox,
    glib::{WeakRef, object::ObjectExt},
    prelude::BoxExt,
};

use crate::ui::{
    g_templates::{notification::NotificationWidget, notification_centre::NotificationCollection},
    widgets::utils::WidgetOption,
};

#[derive(Clone, Debug)]
pub struct NotificationCentre {
    list: WeakRef<ListBox>,
}
impl NotificationCentre {
    pub fn insert(&self, notification: Rc<Notification>) {
        if let Some(list) = self.list.upgrade() {
            let widget = NotificationWidget::new(notification);
            list.append(&widget);
        }
    }
    // pub fn remove(&self, index: u32) {
    //     if let Some(list) = self.list.upgrade() {
    //         list.remove(index);
    //     }
    // }
}

pub struct NotificationCentreBuilder {
    ui: WidgetOption<NotificationCollection>,
    list: WeakRef<ListBox>,
}
impl NotificationCentreBuilder {
    pub fn new() -> Self {
        let collection = NotificationCollection::new();

        let list = ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .hexpand(true)
            .valign(gtk4::Align::Start)
            .build();

        collection.append(&list);

        Self {
            ui: WidgetOption::Owned(collection),
            list: list.downgrade(),
        }
    }
    pub fn for_box(mut self, container: &Box) -> Self {
        if let Some(widget) = self.ui.take() {
            container.append(&widget);
        }
        self
    }
    pub fn build(self) -> NotificationCentre {
        NotificationCentre { list: self.list }
    }
}
