mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk4::subclass::prelude::*;
    use gtk4::{Box as GtkBox, Window};
    use gtk4::{CompositeTemplate, ScrolledWindow, glib};

    use crate::WatsonState;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/dev/skxxtz/watson/ui/window.ui")]
    pub struct MainWindow {
        #[template_child(id = "viewport")]
        pub viewport: TemplateChild<GtkBox>,

        #[template_child(id = "viewport-scroll")]
        pub viewport_scroll: TemplateChild<ScrolledWindow>,

        pub state: Rc<RefCell<WatsonState>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MainWindow {
        const NAME: &'static str = "MainWindow";
        type Type = super::MainWindow;
        type ParentType = Window;

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
}

use gtk4::glib::Object;
use gtk4::prelude::*;

gtk4::glib::wrapper! {
    pub struct MainWindow(ObjectSubclass<imp::MainWindow>)
        @extends gtk4::Widget, gtk4::Window,
        @implements gtk4::Buildable, gtk4::Native, gtk4::Accessible, gtk4::ConstraintTarget, gtk4::ShortcutManager, gtk4::Root;
}

impl MainWindow {
    pub fn new(width: i32, opacity: f64) -> Self {
        let obj: Self = Object::new();
        obj.set_default_width(width);
        obj.set_opacity(opacity);

        obj
    }
}
