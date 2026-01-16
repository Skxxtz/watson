mod imp {
    use gtk4::Box as GtkBox;
    use gtk4::glib;
    use gtk4::subclass::prelude::*;

    #[derive(gtk4::CompositeTemplate, Default)]
    #[template(resource = "/dev/skxxtz/watson/ui/event_details.ui")]
    pub struct EventDetails {
        #[template_child]
        pub event_title: TemplateChild<gtk4::Label>,

        #[template_child]
        pub event_start: TemplateChild<gtk4::Label>,

        #[template_child]
        pub event_end: TemplateChild<gtk4::Label>,

        #[template_child]
        pub event_location: TemplateChild<gtk4::Label>,

        #[template_child]
        pub location_icon: TemplateChild<gtk4::Image>,

        #[template_child]
        pub event_description: TemplateChild<gtk4::Label>,

        #[template_child]
        pub category_label: TemplateChild<gtk4::Label>,

        #[template_child]
        pub recurrence_icon: TemplateChild<gtk4::Image>,

        #[template_child]
        pub recurrence_label: TemplateChild<gtk4::Label>,

        #[template_child]
        pub back_button: TemplateChild<gtk4::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EventDetails {
        const NAME: &'static str = "EventDetails";
        type Type = super::EventDetails;
        type ParentType = GtkBox;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for EventDetails {}
    impl WidgetImpl for EventDetails {}
    impl BoxImpl for EventDetails {}
}

use common::calendar::utils::CalDavEvent;
use common::calendar::utils::structs::DateTimeSpec;
use gtk4::glib::Object;
use gtk4::glib::subclass::types::ObjectSubclassIsExt;
use gtk4::prelude::*;

gtk4::glib::wrapper! {
    pub struct EventDetails(ObjectSubclass<imp::EventDetails>)
        @extends gtk4::Widget,
                 gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
                 gtk4::Native, gtk4::Root, gtk4::ShortcutManager, gtk4::Box;
}

impl EventDetails {
    pub fn new() -> Self {
        let obj: Self = Object::new();
        obj.set_vexpand(true);
        obj.set_hexpand(true);
        obj.set_can_focus(true);
        obj.set_focusable(true);
        obj.set_focus_on_click(true);
        obj.set_can_target(true);

        obj.set_css_classes(&["inner-widget", "calendar-details"]);
        obj
    }
    pub fn set_event(&self, event: &CalDavEvent) {
        let imp = self.imp();

        // 1. Basic Text
        imp.event_title.set_label(&event.title);
        imp.category_label.set_label(&event.calendar_info.name);

        // 2. Date/Time Formatting
        let format_time = |ts: &Option<DateTimeSpec>| {
            ts.as_ref()
                .map(|t| format!("{}", t.local().format("%H:%M"))) // Replace with your actual formatting logic
                .unwrap_or_else(|| "N/A".to_string())
        };
        imp.event_start.set_label(&format_time(&event.start));
        imp.event_end.set_label(&format_time(&event.end));

        // 3. Location (Hide if None)
        if let Some(loc) = &event.location {
            imp.event_location.set_label(loc);
            imp.event_location.set_visible(true);
            imp.location_icon.set_visible(true);
        } else {
            imp.event_location.set_visible(false);
            imp.location_icon.set_visible(false);
        }

        // 4. Recurrence
        if let Some(rule) = &event.recurrence {
            imp.recurrence_label.set_label(&rule.format_str());
            imp.recurrence_label.set_visible(true);
            imp.recurrence_icon.set_visible(true);
        } else {
            imp.recurrence_label.set_visible(false);
            imp.recurrence_icon.set_visible(false);
        }

        // 5. Description
        imp.event_description.set_label(
            event
                .description
                .as_deref()
                .unwrap_or("No additional details."),
        );
    }
}
