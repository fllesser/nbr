use anyhow::Result;
use dialoguer::{Confirm, theme::ColorfulTheme};

#[derive(Default)]
pub struct GlobalContext {
    pub verbose: u8,
    pub yes: bool,
    pub dialoguer_theme: ColorfulTheme,
}

impl GlobalContext {
    pub async fn confirm(&self, message: String, default: bool) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        let confirmed = Confirm::with_theme(&self.dialoguer_theme)
            .with_prompt(message)
            .default(default)
            .interact()?;

        Ok(confirmed)
    }
}
