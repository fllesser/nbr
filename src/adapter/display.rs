use crate::log::StyledText;

use super::registry::RegistryAdapter;

pub fn display_adapter(adapter: &RegistryAdapter) {
    StyledText::new(" ")
        .cyan_bold("  •")
        .cyan_bold(&adapter.name)
        .text(format!("({})", adapter.project_link).as_str())
        .green(format!("v{}", adapter.version).as_str())
        .println();
}
