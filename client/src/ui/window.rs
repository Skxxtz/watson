use gtk4::glib::object::ObjectExt;
use gtk4_layer_shell::LayerShell;

use crate::ui::{WatsonUi, g_templates::main_window::MainWindow};

impl WatsonUi {
    pub fn window(&mut self) -> MainWindow {
        let win = MainWindow::new(100, 1.0);

        win.init_layer_shell();
        win.set_layer(gtk4_layer_shell::Layer::Top);
        win.set_anchor(gtk4_layer_shell::Edge::Top, true);
        win.set_anchor(gtk4_layer_shell::Edge::Right, true);
        win.set_anchor(gtk4_layer_shell::Edge::Bottom, true);

        self.window = win.downgrade();
        win
    }
}
