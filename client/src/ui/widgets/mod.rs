mod battery;
mod button;
pub mod calendar;
mod clock;
mod interactives;
mod notifications;
mod slider;
mod utils;

use std::{cell::RefCell, rc::Rc, sync::Arc};

pub use battery::{Battery, BatteryBuilder};
pub use button::{Button, ButtonBuilder};
pub use calendar::Calendar;
pub use clock::{Clock, HandStyle};
pub use utils::BackendFunc;

use gtk4::{
    Align, AspectFrame, Box, DrawingArea, Separator,
    glib::{WeakRef, object::ObjectExt},
    prelude::{BoxExt, WidgetExt},
};
pub use notifications::{NotificationCentre, NotificationCentreBuilder};
pub use slider::{Slider, SliderBuilder, SliderRange};

use crate::{WatsonState, config::WidgetSpec};

pub fn create_widgets(
    viewport: &Box,
    spec: WidgetSpec,
    state: Rc<RefCell<WatsonState>>,
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
            let calendar = Calendar::builder()
                .for_spec(&spec)
                .for_box(&viewport)
                .build();

            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Calendar(calendar));
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
            let button = {
                ButtonBuilder::new(&spec, Arc::clone(&state.borrow().system_state), in_holder)
                    .for_box(&viewport)
                    .build()
            };
            state
                .borrow_mut()
                .widgets
                .push(WatsonWidget::Button(button));
        }
        WidgetSpec::Slider { .. } => {
            let slider = {
                SliderBuilder::new(&spec, Arc::clone(&state.borrow().system_state), in_holder)
                    .for_box(&viewport)
                    .build()
            };
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
            if let Some(class) = base.class {
                col.add_css_class(&class);
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
                .valign(base.valign.map(|d| d.into()).unwrap_or(Align::Fill))
                .halign(base.halign.map(|d| d.into()).unwrap_or(Align::Fill))
                .hexpand(true)
                .vexpand(true)
                .spacing(spacing)
                .build();

            if let Some(id) = base.id {
                row.set_widget_name(&id);
            }
            if let Some(class) = base.class {
                row.add_css_class(&class);
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
                create_widgets(&row, child, state.clone(), true);
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

macro_rules! define_widgets {
    ($($name:ident($data:ty)),* $(,)?) => {
        #[derive(PartialEq, Copy, Clone, Debug)]
        #[allow(dead_code)]
        pub enum WatsonWidgetType {
            $($name),*
        }

        pub enum WatsonWidget {
            $($name($data)),*
        }

        #[allow(dead_code)]
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
    Calendar(Calendar),
    Clock(WeakRef<DrawingArea>),
    NotificationCentre(NotificationCentre),
    Button(Button),
    Slider(Slider),
}
