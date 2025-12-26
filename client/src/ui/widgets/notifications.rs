use gtk4::{Box, gio::ListModel, glib::WeakRef, prelude::BoxExt};

use crate::ui::{
    g_templates::notification_centre::NotificationCollection, widgets::utils::WidgetOption,
};

pub struct NotificationCentre {
    model: WeakRef<ListModel>,
}

pub struct NotificationCentreBuilder {
    ui: WidgetOption<NotificationCollection>,
}
impl NotificationCentreBuilder {
    pub fn new() -> Self {
        Self {
            ui: WidgetOption::Owned(NotificationCollection::new()),
        }
    }
    pub fn for_box(mut self, container: &Box) -> Self {
        if let Some(widget) = self.ui.take() {
            container.append(&widget);
        }
        self
    }
    // pub fn build(self) -> NotificationCentre {
    // }
}
