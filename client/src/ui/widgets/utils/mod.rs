use gtk4::glib::{
    WeakRef,
    object::{ObjectExt, ObjectType},
};

pub mod animation;
pub mod backend_functions;
pub mod interactives;
pub mod render;

pub enum WidgetOption<T: ObjectType> {
    Borrowed(WeakRef<T>),
    Owned(T),
}
impl<T: ObjectType> Default for WidgetOption<T> {
    fn default() -> Self {
        Self::Borrowed(WeakRef::new())
    }
}
impl<T: ObjectType> WidgetOption<T> {
    /// Take the owned value and replace it with a weak reference
    pub fn take(&mut self) -> Option<T> {
        match std::mem::replace(self, WidgetOption::Borrowed(WeakRef::default())) {
            WidgetOption::Owned(obj) => {
                *self = WidgetOption::Borrowed(obj.downgrade());
                Some(obj)
            }
            WidgetOption::Borrowed(weak) => weak.upgrade(),
        }
    }
    pub fn downgrade(&self) -> WeakRef<T> {
        match self {
            Self::Owned(obj) => obj.downgrade(),
            Self::Borrowed(b) => b.clone(),
        }
    }
}
