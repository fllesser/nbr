use anyhow::Result;
use dialoguer::MultiSelect;
use dialoguer::theme::ColorfulTheme;
use std::collections::HashSet;
use tracing::{error, info, warn};

use crate::log::StyledText;
use crate::pyproject::{Adapter, NbTomlEditor};
use crate::uv;

use super::AdapterManager;

impl AdapterManager {
    pub async fn install_adapters(&self, fetch_remote: bool) -> Result<()> {
        let selected_adapters = self.select_adapters(fetch_remote, true).await?;

        if selected_adapters.is_empty() {
            warn!("You haven't selected any adapters to install");
            return Ok(());
        }
        let selected_adapters_names = selected_adapters
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<String>>()
            .join(", ");
        let prompt = StyledText::new(" ")
            .white_bold("Would you like to install")
            .cyan_bold(format!("[{}]", selected_adapters_names).as_str())
            .to_string();

        if self.global_context.confirm(prompt, true).await? {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        let adapter_packages = selected_adapters
            .iter()
            .map(|a| a.project_link.as_str())
            .collect::<HashSet<&str>>()
            .into_iter()
            .collect::<Vec<&str>>();

        uv::add(adapter_packages)
            .working_dir(&self.work_dir)
            .run()?;
        let adapters = selected_adapters
            .iter()
            .map(|a| (*a).into())
            .collect::<Vec<Adapter>>();
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?.add_adapters(adapters)?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed adapters:")
            .cyan_bold(&selected_adapters_names)
            .println();
        Ok(())
    }

    pub async fn uninstall_adapters(&self) -> Result<()> {
        let mut installed_adapters = self.get_installed_adapters_names();
        if installed_adapters.is_empty() {
            warn!("You haven't installed any adapters");
            return Ok(());
        }

        let selected_adapters: Vec<&str> = {
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Select installed adapter(s) to uninstall")
                .items(&installed_adapters)
                .interact()?;
            selections
                .into_iter()
                .map(|i| installed_adapters[i])
                .collect()
        };

        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .remove_adapters(selected_adapters.to_vec())?;

        let registry_adapters = self.fetch_registry_adapters(false).await?;
        let mut adapter_packages = selected_adapters
            .iter()
            .filter_map(|name| {
                registry_adapters
                    .get(*name)
                    .map(|a| a.project_link.as_str())
            })
            .collect::<HashSet<&str>>()
            .into_iter()
            .collect::<Vec<&str>>();

        installed_adapters.retain(|name| !selected_adapters.contains(name));
        if installed_adapters
            .iter()
            .any(|name| name.starts_with("OneBot"))
        {
            adapter_packages.retain(|name| *name != "nonebot-adapter-onebot");
        }

        if !adapter_packages.is_empty() {
            uv::remove(adapter_packages)
                .working_dir(&self.work_dir)
                .run()?;
        }

        StyledText::new(" ")
            .green_bold("✓ Successfully uninstalled adapters:")
            .cyan_bold(selected_adapters.join(", "))
            .println();
        Ok(())
    }

    pub async fn list_adapters(&self, show_all: bool) -> Result<()> {
        let installed_adapters = self.get_installed_adapters_names();
        let adapters_map = self.fetch_registry_adapters(show_all).await?;

        if show_all {
            info!("All Adapters:");
            adapters_map
                .iter()
                .for_each(|(_, adapter)| self.display_adapter(adapter));
        } else {
            if installed_adapters.is_empty() {
                warn!("No adapters installed.");
                return Ok(());
            }
            info!("Installed Adapters:");
            installed_adapters.iter().for_each(|name| {
                if let Some(adapter) = adapters_map.get(*name) {
                    self.display_adapter(adapter);
                }
            });
        }
        Ok(())
    }
}
