use crate::auth::AuthTui;

pub struct ArgParse;
impl ArgParse {
    pub fn parse(args: std::env::Args) {
        let args = args.skip(1).peekable();
        for arg in args {
            match arg.as_str() {
                "auth" => {
                    let mut tui = AuthTui::new();
                    tui.run();
                }
                _ => {}
            }
        }
    }
}
