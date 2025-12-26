// src/ui/templates/notification_obj.rs

use std::rc::Rc;

use common::notification::Notification;
use gtk4::glib::subclass::prelude::*;
use gtk4::glib::{self, Object};

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    pub struct NotificationObjImp {
        pub notification: Rc<RefCell<Notification>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NotificationObjImp {
        const NAME: &'static str = "NotificationObj";
        type Type = super::NotificationObj;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for NotificationObjImp {}
}

// Only one wrapper
glib::wrapper! {
    pub struct NotificationObj(ObjectSubclass<imp::NotificationObjImp>);
}

impl NotificationObj {
    pub fn new() -> Self {
        let obj: Self = Object::new();
        obj
    }
}
