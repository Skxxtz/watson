mod imp {
    use gtk4::Box as GtkBox;
    use gtk4::glib;
    use gtk4::subclass::prelude::*;

    #[derive(gtk4::CompositeTemplate, Default)]
    #[template(resource = "/dev/skxxtz/watson/ui/notification_centre.ui")]
    pub struct NotificationCollection {}

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

use gtk4::gio::{ActionGroup, ActionMap, ListStore};
use gtk4::glib::object::{Cast, CastNone, ObjectExt};
use gtk4::glib::{Object, WeakRef};
use gtk4::prelude::{BoxExt, ListItemExt, WidgetExt};
use gtk4::{
    Box, CustomFilter, FilterListModel, Image, Label, ListBox, ListItem, SignalListItemFactory,
    SingleSelection,
};

use crate::ui::g_templates::notification_obj::NotificationObj;

gtk4::glib::wrapper! {
    pub struct NotificationCollection(ObjectSubclass<imp::NotificationCollection>)
        @extends gtk4::Widget, gtk4::Box,
                 gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
                 ActionMap, ActionGroup, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl NotificationCollection {
    pub fn new() -> Self {
        let obj: Self = Object::new();

        let list = ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .hexpand(true)
            .valign(gtk4::Align::Start)
            .build();

        obj.append(&list);
        obj.set_css_classes(&["widget", "notification_centre"]);
        obj.set_vexpand(false);
        obj.set_hexpand(true);
        obj.set_height_request(-1);
        obj.set_valign(gtk4::Align::Start);

        obj
    }
    pub fn construct_liststore() -> (SingleSelection, SignalListItemFactory, WeakRef<ListStore>) {
        let store = ListStore::new::<NotificationObj>();
        let store_weak = store.downgrade();

        let filter = CustomFilter::new(move |item| {
            let notification_obj = item
                .downcast_ref::<NotificationObj>()
                .expect("Expected NotificationObj");
            notification_obj.notification().is_some()
        });
        let filter_model = FilterListModel::new(Some(store), Some(filter));
        let selection_model = SingleSelection::new(Some(filter_model));

        let factory = SignalListItemFactory::new();

        factory.connect_setup(|_, list_item| {
            let list_item = list_item
                .downcast_ref::<ListItem>()
                .expect("Expected a ListItem");

            let container = Box::builder()
                .spacing(4)
                .hexpand(true)
                .halign(gtk4::Align::Fill)
                .css_classes(["notification"])
                .build();

            let status_icon = Image::builder().css_classes(["status-icon"]).build();

            // Construcs the content box
            let content = Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(4)
                .hexpand(true)
                .halign(gtk4::Align::Fill)
                .build();

            // Title box
            let title_box = Box::builder().spacing(10).hexpand(true).build();
            let app_icon = Image::builder()
                .css_classes(["app-icon"])
                .halign(gtk4::Align::Start)
                .valign(gtk4::Align::Center)
                .build();
            let title = Label::builder()
                .css_classes(["notification-title"])
                .wrap(true)
                .wrap_mode(gtk4::pango::WrapMode::Word)
                .halign(gtk4::Align::Start)
                .valign(gtk4::Align::Center)
                .build();
            title_box.append(&app_icon);
            title_box.append(&title);
            content.append(&title_box);

            // Body
            let body_box = Box::builder()
                .hexpand(true)
                .halign(gtk4::Align::Fill)
                .build();

            let body = Label::builder()
                .css_classes(["notification-body", "text-body"])
                .wrap(true)
                .wrap_mode(gtk4::pango::WrapMode::Word)
                .valign(gtk4::Align::End)
                .halign(gtk4::Align::Start)
                .xalign(0.0)
                .build();
            body_box.append(&body);
            content.append(&body_box);

            // Actions
            let actions = Box::builder()
                .spacing(4)
                .valign(gtk4::Align::End)
                .css_classes(["notification-actions"])
                .build();
            content.append(&actions);

            let close_icon = Image::builder()
                .css_classes(["close-icon"])
                .valign(gtk4::Align::Start)
                .icon_name("close")
                .build();

            // Populate
            container.append(&status_icon);
            container.append(&content);
            container.append(&close_icon);

            list_item.set_child(Some(&container));
        });

        factory.connect_bind(|_, list_item| {
            let list_item = list_item
                .downcast_ref::<ListItem>()
                .expect("Expected a ListItem");

            let container = list_item
                .child()
                .and_downcast::<gtk4::Box>()
                .expect("Expected a GtkBox");

            let _status_icon = container.first_child().and_downcast::<Image>().unwrap();
            let content = container
                .first_child()
                .and_then(|s| s.next_sibling())
                .and_downcast::<Box>()
                .unwrap();
            let _close_icon = container.last_child().and_downcast::<Image>().unwrap();

            let title_box = content.first_child().and_downcast::<Box>().unwrap();
            let body_box = title_box.next_sibling().and_downcast::<Box>().unwrap();
            let body = body_box.first_child().and_downcast::<Label>().unwrap();
            let actions = content.last_child().and_downcast::<Box>().unwrap();

            let app_icon = title_box.first_child().and_downcast::<Image>().unwrap();
            let title = title_box.last_child().and_downcast::<Label>().unwrap();

            while let Some(action) = actions.last_child() {
                actions.remove(&action);
            }

            let notification_obj = list_item
                .item()
                .and_downcast::<NotificationObj>()
                .expect("Expected item.");

            // Should never panic due to filter
            let notification = notification_obj.notification().unwrap();

            container.add_css_class(notification.urgency.css_class());

            title.set_visible(!&notification.summary.is_empty());
            title.set_text(&notification.summary);

            body.set_visible(!&notification.body.is_empty());
            body.set_text(&notification.body);

            app_icon.set_visible(!notification.app_icon.is_empty());
            app_icon.set_icon_name(Some(&notification.app_icon));

            body_box.set_size_request(1, -1);
            content.set_size_request(1, -1);
        });

        (selection_model, factory, store_weak)
    }
}
