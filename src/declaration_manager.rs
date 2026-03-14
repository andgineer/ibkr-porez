use std::path::Path;

use anyhow::{Result, bail};
use chrono::Local;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::models::{Declaration, DeclarationStatus, DeclarationType};
use crate::storage::Storage;

pub struct ExportResult {
    pub xml_path: Option<String>,
    pub attachment_paths: Vec<String>,
}

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

        decl.metadata.insert(
            "assessed_tax_due_rsd".into(),
            format!("{tax_due:.2}").into(),
        );
        decl.metadata
            .insert("tax_due_rsd".into(), format!("{tax_due:.2}").into());

        let now = Local::now().naive_local();
        if decl.submitted_at.is_none() {
            decl.submitted_at = Some(now);
        }

        if tax_due == Decimal::ZERO || mark_paid {
            decl.status = DeclarationStatus::Finalized;
            decl.paid_at = Some(now);
        } else {
            decl.status = DeclarationStatus::Submitted;
        }

        self.storage.save_declaration(&decl)?;
        Ok(())
    }

    pub fn export(&self, id: &str, output_dir: &Path) -> Result<ExportResult> {
        let decl = self.get_or_err(id)?;
        std::fs::create_dir_all(output_dir)?;
        let mut result = ExportResult {
            xml_path: None,
            attachment_paths: Vec::new(),
        };

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
            result.xml_path = Some(dest.display().to_string());
        } else if let Some(ref fp) = decl.file_path {
            let src = Path::new(fp);
            if src.exists() {
                let filename = src
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&default_name);
                let dest = output_dir.join(filename);
                std::fs::copy(src, &dest)?;
                result.xml_path = Some(dest.display().to_string());
            }
        }

        let decl_dir = self.storage.declarations_dir();
        for (_, attachment_path) in &decl.attached_files {
            let src = decl_dir.join(attachment_path);
            if src.exists()
                && let Some(name) = src.file_name()
            {
                let dest = output_dir.join(name);
                std::fs::copy(src, &dest)?;
                result.attachment_paths.push(dest.display().to_string());
            }
        }

        Ok(result)
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

        let relative_path = Path::new(id).join("attachments").join(&file_name);
        decl.attached_files
            .insert(file_name.clone(), relative_path.display().to_string());
        self.storage.save_declaration(&decl)?;

        Ok(file_name)
    }

    pub fn detach_file(&self, id: &str, file_id: &str) -> Result<()> {
        let mut decl = self.get_or_err(id)?;

        let Some(rel_path) = decl.attached_files.shift_remove(file_id) else {
            bail!("file '{file_id}' not found in attachments for declaration {id}");
        };

        let full_path = self.storage.declarations_dir().join(&rel_path);
        let _ = std::fs::remove_file(&full_path);

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

    #[must_use]
    pub fn assessment_message(
        id: &str,
        tax_due: Decimal,
        status: DeclarationStatus,
        mark_paid: bool,
    ) -> String {
        if mark_paid {
            format!("Assessment saved and paid: {id} ({tax_due} RSD)")
        } else if status == DeclarationStatus::Finalized {
            format!("Assessment saved: {id} (no tax to pay)")
        } else {
            format!("Assessment saved: {id} ({tax_due} RSD to pay)")
        }
    }

    fn get_or_err(&self, id: &str) -> Result<Declaration> {
        self.storage
            .get_declaration(id)
            .ok_or_else(|| anyhow::anyhow!("declaration {id} not found"))
    }
}
