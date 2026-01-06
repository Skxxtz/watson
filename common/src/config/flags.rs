use crate::{auth::AuthTui, errors::WatsonError};

pub struct ArgParse;
impl ArgParse {
    pub async fn parse(args: std::env::Args) -> Result<(), WatsonError> {
        let args = args.skip(1).peekable();
        for arg in args {
            match arg.as_str() {
                "auth" => {
                    let mut tui = AuthTui::new()?;
                    tui.run().await?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
