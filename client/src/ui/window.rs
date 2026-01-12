use gtk4::{Application, glib::object::ObjectExt};
use gtk4_layer_shell::LayerShell;

use crate::ui::{WatsonUi, g_templates::main_window::MainWindow};

impl WatsonUi {
    pub fn window(&mut self, app: &Application) -> MainWindow {
        let win = MainWindow::new(app, 100, 1.0);

        win.init_layer_shell();
        win.set_layer(gtk4_layer_shell::Layer::Top);
        win.set_anchor(gtk4_layer_shell::Edge::Top, true);
        win.set_anchor(gtk4_layer_shell::Edge::Right, true);

        self.window = win.downgrade();
        win
    }
}
