use gtk4::{
    EventControllerKey, PropagationPhase,
    glib::object::ObjectExt,
    prelude::{EventControllerExt, GtkWindowExt, WidgetExt},
};
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
        win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
        win.set_exclusive_zone(0);

        let controller = EventControllerKey::new();
        controller.set_propagation_phase(PropagationPhase::Bubble);
        controller.connect_key_pressed({
            let win = win.downgrade();
            move |_gesture, key, _keycode, _state| {
                if key == gtk4::gdk::Key::Escape {
                    if let Some(win) = win.upgrade() {
                        win.close();
                        return gtk4::glib::Propagation::Stop;
                    }
                }
                gtk4::glib::Propagation::Proceed
            }
        });
        win.add_controller(controller);

        self.window = win.downgrade();
        win
    }
}
