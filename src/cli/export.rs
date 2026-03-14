use std::path::PathBuf;

use anyhow::Result;

use super::{load_config_or_exit, make_storage, output};
use ibkr_porez::config as app_config;
use ibkr_porez::declaration_manager::DeclarationManager;

pub fn run(declaration_id: &str, output_dir: Option<PathBuf>) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    let dest = output_dir.unwrap_or_else(|| app_config::get_effective_output_dir_path(&cfg));
    let exported = manager.export(declaration_id, &dest)?;

    if let Some(ref xml) = exported.xml_path {
        output::success(&format!("Exported XML: {xml}"));
    }
    if !exported.attachment_paths.is_empty() {
        output::success(&format!(
            "Exported {} attached file(s):",
            exported.attachment_paths.len()
        ));
        for path in &exported.attachment_paths {
            println!("  {path}");
        }
    }
    if exported.xml_path.is_none() && exported.attachment_paths.is_empty() {
        output::warning(&format!(
            "No files to export for declaration {declaration_id}."
        ));
    }
    Ok(())
}
