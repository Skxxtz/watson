mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk4::{ScrolledWindow, glib};
    use gtk4::subclass::prelude::*;
    use gtk4::{ApplicationWindow, Box as GtkBox};

    use crate::UiState;

    #[derive(gtk4::CompositeTemplate, Default)]
    #[template(resource = "/dev/skxxtz/watson/ui/window.ui")]
    pub struct MainWindow {
        #[template_child(id = "viewport")]
        pub viewport: TemplateChild<GtkBox>,

        #[template_child(id = "viewport-scroll")]
        pub viewport_scroll: TemplateChild<ScrolledWindow>,

        pub state: Rc<RefCell<UiState>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MainWindow {
        const NAME: &'static str = "MainWindow";
        type Type = super::MainWindow;
        type ParentType = ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MainWindow {}
    impl WidgetImpl for MainWindow {}
    impl WindowImpl for MainWindow {}
    impl ApplicationWindowImpl for MainWindow {}
}

use gtk4::gdk::{Display, Monitor};
use gtk4::gio::{ActionGroup, ActionMap};
use gtk4::glib::Object;
use gtk4::glib::subclass::types::ObjectSubclassIsExt;
use gtk4::prelude::*;

gtk4::glib::wrapper! {
    pub struct MainWindow(ObjectSubclass<imp::MainWindow>)
        @extends gtk4::ApplicationWindow, gtk4::Window, gtk4::Widget,
                 gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
                 ActionMap, ActionGroup, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl MainWindow {
    pub fn new(application: &gtk4::Application, width: i32, opacity: f64) -> Self {
        let obj: Self = Object::new();
        obj.set_application(Some(application));
        obj.set_default_width(width);
        obj.set_opacity(opacity);

        let imp = obj.imp();
        if let Some(display) = Display::default() {
            let monitors = display.monitors();
            if let Some(monitor) = monitors.item(0).and_downcast::<Monitor>() {
                let geo = monitor.geometry();
                let max_h = (geo.height() as f32 * 0.9) as i32;
                imp.viewport_scroll.set_max_content_height(max_h);
            }
        }


        obj

    }
}
