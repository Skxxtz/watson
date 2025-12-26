mod imp {
    use gtk4::Box as GtkBox;
    use gtk4::subclass::prelude::*;
    use gtk4::{ListBox, glib};

    #[derive(gtk4::CompositeTemplate, Default)]
    #[template(resource = "/dev/skxxtz/watson/ui/notification_centre.ui")]
    pub struct NotificationCollection {
        #[template_child(id = "list")]
        pub listview: TemplateChild<ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NotificationCollection {
        const NAME: &'static str = "NotificationCollection";
        type Type = super::NotificationCollection;
        type ParentType = GtkBox;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for NotificationCollection {}
    impl WidgetImpl for NotificationCollection {}
    impl BoxImpl for NotificationCollection {}
}

use gtk4::gio::{ActionGroup, ActionMap};
use gtk4::glib::Object;
use gtk4::glib::subclass::types::ObjectSubclassIsExt;
use gtk4::prelude::WidgetExt;

gtk4::glib::wrapper! {
    pub struct NotificationCollection(ObjectSubclass<imp::NotificationCollection>)
        @extends gtk4::Widget, gtk4::Box,
                 gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
                 ActionMap, ActionGroup, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl NotificationCollection {
    pub fn new() -> Self {
        let obj: Self = Object::new();
        obj.set_css_classes(&["widget", "notification_centre"]);

        let imp = obj.imp();
        imp.listview.set_vexpand(false);
        imp.listview.set_hexpand(true);
        imp.listview.set_height_request(0);
        imp.listview.set_valign(gtk4::Align::Start);
        imp.listview.set_halign(gtk4::Align::Start);

        obj.set_vexpand(false);
        obj.set_height_request(0);
        obj.set_valign(gtk4::Align::Start);

        obj
    }
}
