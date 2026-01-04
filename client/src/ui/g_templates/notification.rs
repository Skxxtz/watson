mod imp {
    use gtk4::Box as GtkBox;
    use gtk4::Image;
    use gtk4::Label;
    use gtk4::glib;
    use gtk4::subclass::prelude::*;

    #[derive(gtk4::CompositeTemplate, Default)]
    #[template(resource = "/dev/skxxtz/watson/ui/notification.ui")]
    pub struct NotificationWidget {
        #[template_child(id = "title")]
        pub title: TemplateChild<Label>,

        #[template_child(id = "body")]
        pub body: TemplateChild<Label>,

        #[template_child(id = "app_icon")]
        pub app_icon: TemplateChild<Image>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NotificationWidget {
        const NAME: &'static str = "Notification";
        type Type = super::NotificationWidget;
        type ParentType = GtkBox;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for NotificationWidget {}
    impl WidgetImpl for NotificationWidget {}
    impl BoxImpl for NotificationWidget {}
}

use std::rc::Rc;

use common::notification::Notification;
use gtk4::gio::{ActionGroup, ActionMap};
use gtk4::glib::Object;
use gtk4::glib::subclass::types::ObjectSubclassIsExt;
use gtk4::prelude::WidgetExt;

gtk4::glib::wrapper! {
    pub struct NotificationWidget(ObjectSubclass<imp::NotificationWidget>)
        @extends gtk4::Widget, gtk4::Box,
                 gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
                 ActionMap, ActionGroup, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl NotificationWidget {
    pub fn new(notification: Rc<Notification>) -> Self {
        let obj: Self = Object::new();

        obj.set_css_classes(&["notification"]);
        obj.set_vexpand(false);
        obj.set_hexpand(true);
        obj.set_height_request(-1);
        obj.set_valign(gtk4::Align::Start);

        // Notification
        let imp = obj.imp();

        // Handle visibility
        imp.title.set_visible(!&notification.summary.is_empty());
        imp.body.set_visible(!&notification.body.is_empty());
        imp.app_icon.set_visible(!&notification.app_icon.is_empty());

        // Populate values
        imp.title.set_text(&notification.summary);
        imp.body.set_text(&notification.body);
        imp.app_icon.set_icon_name(Some(&notification.app_icon));

        obj.add_css_class(notification.urgency.css_class());

        obj
    }
}
