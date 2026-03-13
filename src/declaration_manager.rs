use std::path::Path;

use anyhow::{Result, bail};
use chrono::Local;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::models::{Declaration, DeclarationStatus, DeclarationType};
use crate::storage::Storage;

pub struct DeclarationManager<'a> {
    storage: &'a Storage,
}

impl<'a> DeclarationManager<'a> {
    #[must_use]
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    pub fn submit(&self, ids: &[&str]) -> Result<()> {
        for id in ids {
            let mut decl = self.get_or_err(id)?;
            if decl.status != DeclarationStatus::Draft {
                bail!("declaration {id} is not in Draft status");
            }

            let now = Local::now().naive_local();
            let target = match decl.r#type {
                DeclarationType::Ppdg3r => DeclarationStatus::Pending,
                DeclarationType::Ppo => {
                    let due = self.tax_due_rsd(&decl);
                    if due > Decimal::ZERO {
                        DeclarationStatus::Submitted
                    } else {
                        DeclarationStatus::Finalized
                    }
                }
            };
            decl.status = target;
            if decl.submitted_at.is_none() {
                decl.submitted_at = Some(now);
            }
            self.storage.save_declaration(&decl)?;
        }
        Ok(())
    }

    pub fn pay(&self, ids: &[&str]) -> Result<()> {
        for id in ids {
            let mut decl = self.get_or_err(id)?;
            match decl.status {
                DeclarationStatus::Draft
                | DeclarationStatus::Submitted
                | DeclarationStatus::Pending => {}
                DeclarationStatus::Finalized => {
                    bail!("declaration {id} is already finalized");
                }
            }
            let now = Local::now().naive_local();
            decl.status = DeclarationStatus::Finalized;
            if decl.submitted_at.is_none() {
                decl.submitted_at = Some(now);
            }
            decl.paid_at = Some(now);
            self.storage.save_declaration(&decl)?;
        }
        Ok(())
    }

    pub fn set_assessed_tax(&self, id: &str, tax_due: Decimal, mark_paid: bool) -> Result<()> {
        let mut decl = self.get_or_err(id)?;
        if decl.status == DeclarationStatus::Draft {
            bail!("cannot set assessed tax on a Draft declaration");
        }

        decl.metadata
            .insert("assessed_tax_due_rsd".into(), tax_due.to_string().into());
        decl.metadata
            .insert("tax_due_rsd".into(), tax_due.to_string().into());

        if mark_paid {
            let now = Local::now().naive_local();
            decl.status = DeclarationStatus::Finalized;
            decl.paid_at = Some(now);
        }
        self.storage.save_declaration(&decl)?;
        Ok(())
    }

    pub fn export(&self, id: &str, output_dir: &Path) -> Result<Vec<String>> {
        let decl = self.get_or_err(id)?;
        std::fs::create_dir_all(output_dir)?;
        let mut exported = Vec::new();

        let default_name = format!("declaration-{id}.xml");
        if let Some(ref xml) = decl.xml_content {
            let filename = decl
                .file_path
                .as_deref()
                .and_then(|p| Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or(&default_name);
            let dest = output_dir.join(filename);
            std::fs::write(&dest, xml)?;
            exported.push(dest.display().to_string());
        } else if let Some(ref fp) = decl.file_path {
            let src = Path::new(fp);
            if src.exists() {
                let filename = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&default_name);
                let dest = output_dir.join(filename);
                std::fs::copy(src, &dest)?;
                exported.push(dest.display().to_string());
            }
        }

        for (_, attachment_path) in &decl.attached_files {
            let src = Path::new(attachment_path);
            if src.exists()
                && let Some(name) = src.file_name()
            {
                let dest = output_dir.join(name);
                std::fs::copy(src, &dest)?;
                exported.push(dest.display().to_string());
            }
        }

        Ok(exported)
    }

    pub fn revert(&self, ids: &[&str]) -> Result<()> {
        for id in ids {
            let mut decl = self.get_or_err(id)?;
            decl.status = DeclarationStatus::Draft;
            decl.submitted_at = None;
            decl.paid_at = None;
            self.storage.save_declaration(&decl)?;
        }
        Ok(())
    }

    pub fn attach_file(&self, id: &str, path: &Path) -> Result<String> {
        let mut decl = self.get_or_err(id)?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid file path"))?
            .to_string();

        let attachments_dir = self.storage.declarations_dir().join(id).join("attachments");
        std::fs::create_dir_all(&attachments_dir)?;

        let dest = attachments_dir.join(&file_name);
        std::fs::copy(path, &dest)?;

        decl.attached_files
            .insert(file_name.clone(), dest.display().to_string());
        self.storage.save_declaration(&decl)?;

        Ok(file_name)
    }

    pub fn detach_file(&self, id: &str, file_id: &str) -> Result<()> {
        let mut decl = self.get_or_err(id)?;

        if let Some(path) = decl.attached_files.shift_remove(file_id) {
            let _ = std::fs::remove_file(&path);
        }

        self.storage.save_declaration(&decl)?;
        Ok(())
    }

    /// Resolve the effective `tax_due_rsd`:
    /// assessed > `tax_due` > default(1.00).
    #[must_use]
    pub fn tax_due_rsd(&self, decl: &Declaration) -> Decimal {
        if let Some(v) = decl.metadata.get("assessed_tax_due_rsd")
            && let Some(s) = v.as_str()
            && let Ok(d) = Decimal::from_str(s)
        {
            return d;
        }
        if let Some(v) = decl.metadata.get("tax_due_rsd")
            && let Some(s) = v.as_str()
            && let Ok(d) = Decimal::from_str(s)
        {
            return d;
        }
        Decimal::ONE
    }

    fn get_or_err(&self, id: &str) -> Result<Declaration> {
        self.storage
            .get_declaration(id)
            .ok_or_else(|| anyhow::anyhow!("declaration {id} not found"))
    }
}
