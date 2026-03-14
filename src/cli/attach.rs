use std::path::PathBuf;

use anyhow::{Result, bail};

use super::{load_config_or_exit, make_storage, output};
use ibkr_porez::declaration_manager::DeclarationManager;

pub fn run(
    declaration_id: &str,
    file_path: Option<PathBuf>,
    delete: bool,
    file_id: Option<String>,
) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    if delete {
        let identifier = file_id
            .or_else(|| {
                file_path
                    .as_ref()
                    .and_then(|p| p.to_str().map(String::from))
            })
            .ok_or_else(|| anyhow::anyhow!("file identifier required for deletion"))?;
        manager.detach_file(declaration_id, &identifier)?;
        output::success(&format!(
            "Removed file '{identifier}' from declaration {declaration_id}"
        ));
    } else {
        let path = file_path.ok_or_else(|| anyhow::anyhow!("file path required for attachment"))?;
        if !path.exists() {
            bail!("file not found: {}", path.display());
        }
        let name = manager.attach_file(declaration_id, &path)?;
        output::success(&format!(
            "Attached file '{name}' to declaration {declaration_id}"
        ));
    }

    Ok(())
}
