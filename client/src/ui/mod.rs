use gtk4::glib::WeakRef;

use crate::ui::g_templates::main_window::MainWindow;

mod g_templates;
pub mod utils;
pub mod widgets;
mod window;

#[derive(Default)]
pub struct WatsonUi {
    pub window: WeakRef<MainWindow>,
}
