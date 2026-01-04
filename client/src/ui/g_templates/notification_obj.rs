// src/ui/templates/notification_obj.rs

use std::cell::RefCell;
use std::rc::Rc;

use common::notification::Notification;
use gtk4::glib::subclass::prelude::*;
use gtk4::glib::{self, Object};

mod imp {
    use std::rc::Weak;

    use super::*;

    #[derive(Default)]
    pub struct NotificationObjImp {
        pub notification: RefCell<Weak<Notification>>,
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
    pub fn new(notification: Rc<Notification>) -> Self {
        let obj: Self = Object::new();
        let imp = obj.imp();

        imp.notification.replace(Rc::downgrade(&notification));

        obj
    }

    pub fn notification(&self) -> Option<Rc<Notification>> {
        self.imp().notification.borrow().upgrade()
    }
}
