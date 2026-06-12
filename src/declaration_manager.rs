use std::path::Path;

use anyhow::{Result, bail};
use chrono::{Datelike, Local, NaiveDate};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::models::{CarryforwardVintage, Declaration, DeclarationStatus, DeclarationType};
use crate::storage::Storage;

pub struct ExportResult {
    pub xml_path: Option<String>,
    pub attachment_paths: Vec<String>,
}

pub struct BulkResult {
    pub ok_count: usize,
    pub errors: Vec<(String, String)>,
}

impl BulkResult {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    #[must_use]
    pub fn error_summary(&self) -> String {
        self.errors
            .iter()
            .map(|(id, msg)| format!("{id}: {msg}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Input to [`DeclarationManager::record_assessment`]. All monetary fields
/// are optional individually, but at least one of `assessed_tax_rsd`,
/// `recognized_capital_gain_rsd`, or `recognized_capital_loss_rsd` must be set.
#[derive(Default)]
pub struct AssessmentInput {
    pub assessed_tax_rsd: Option<Decimal>,
    pub recognized_capital_gain_rsd: Option<Decimal>,
    pub recognized_capital_loss_rsd: Option<Decimal>,
    pub assessment_reference: Option<String>,
    pub assessment_date: Option<NaiveDate>,
    pub assessment_notes: Option<String>,
    pub mark_paid: bool,
}

/// Read a `Decimal` previously stored as a formatted string in `decl.metadata`.
fn metadata_decimal(decl: &Declaration, key: &str) -> Option<Decimal> {
    decl.metadata
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Decimal::from_str(s).ok())
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

    pub fn record_assessment(&self, id: &str, input: &AssessmentInput) -> Result<()> {
        if input.assessed_tax_rsd.is_none()
            && input.recognized_capital_gain_rsd.is_none()
            && input.recognized_capital_loss_rsd.is_none()
        {
            bail!("at least one of --tax, --gain, or --loss must be provided");
        }

        let mut decl = self.get_or_err(id)?;
        if decl.status == DeclarationStatus::Draft {
            bail!("cannot record assessment on a Draft declaration");
        }
        if (input.recognized_capital_gain_rsd.is_some()
            || input.recognized_capital_loss_rsd.is_some())
            && decl.r#type != DeclarationType::Ppdg3r
        {
            bail!("recognized capital gain/loss only applies to PPDG-3R declarations");
        }

        // Compare against the *resulting* recognized gain/loss (this input
        // merged onto whatever a prior assessment already stored), not just
        // the new input in isolation -- otherwise two separate `assess`
        // calls (one recognizing a gain, the other a loss) could leave the
        // declaration recognizing both at once.
        let effective_gain = input
            .recognized_capital_gain_rsd
            .or_else(|| metadata_decimal(&decl, "recognized_capital_gain_rsd"));
        let effective_loss = input
            .recognized_capital_loss_rsd
            .or_else(|| metadata_decimal(&decl, "recognized_capital_loss_rsd"));
        if matches!(effective_gain, Some(g) if g > Decimal::ZERO)
            && matches!(effective_loss, Some(l) if l > Decimal::ZERO)
        {
            bail!(
                "a single assessment cannot recognize both a capital gain and a capital loss \
                 (declaration {id} already has the other recognized; pass 0 to clear it first)"
            );
        }

        if let Some(tax_due) = input.assessed_tax_rsd {
            decl.metadata.insert(
                "assessed_tax_due_rsd".into(),
                format!("{tax_due:.2}").into(),
            );
            decl.metadata
                .insert("tax_due_rsd".into(), format!("{tax_due:.2}").into());
        }
        if let Some(g) = input.recognized_capital_gain_rsd {
            decl.metadata.insert(
                "recognized_capital_gain_rsd".into(),
                format!("{g:.2}").into(),
            );
        }
        if let Some(l) = input.recognized_capital_loss_rsd {
            decl.metadata.insert(
                "recognized_capital_loss_rsd".into(),
                format!("{l:.2}").into(),
            );
        }
        if let Some(r) = &input.assessment_reference {
            decl.metadata
                .insert("assessment_reference".into(), r.clone().into());
        }
        if let Some(d) = input.assessment_date {
            decl.metadata.insert(
                "assessment_date".into(),
                d.format("%Y-%m-%d").to_string().into(),
            );
        }
        if let Some(n) = &input.assessment_notes {
            decl.metadata
                .insert("assessment_notes".into(), n.clone().into());
        }

        let now = Local::now().naive_local();
        if decl.submitted_at.is_none() {
            decl.submitted_at = Some(now);
        }
        if let Some(tax_due) = input.assessed_tax_rsd {
            if tax_due == Decimal::ZERO || input.mark_paid {
                decl.status = DeclarationStatus::Finalized;
                decl.paid_at = Some(now);
            } else {
                decl.status = DeclarationStatus::Submitted;
            }
        } else if input.mark_paid {
            decl.status = DeclarationStatus::Finalized;
            decl.paid_at = Some(now);
        }

        // Sync the ledger *before* persisting the declaration: if this bails
        // (e.g. vintage already partially consumed), the declaration metadata
        // must not be saved either, so `declarations.json` and
        // `capital_losses.json` never disagree about the recognized loss.
        self.sync_carryforward_vintage(&decl, input)?;
        self.storage.save_declaration(&decl)?;
        Ok(())
    }

    /// Remove the carryforward vintage created for `decl` (if any), unless it
    /// has already been partially or fully consumed -- in which case we bail
    /// rather than silently leave the ledger inconsistent with declarations
    /// that already drew from it.
    fn remove_unconsumed_carryforward_vintage(&self, decl: &Declaration) -> Result<()> {
        if decl.r#type != DeclarationType::Ppdg3r {
            return Ok(());
        }
        let vintage_id = format!("CF-{}", decl.declaration_id);
        let Some(v) = self.storage.find_carryforward_vintage(&vintage_id) else {
            return Ok(());
        };
        if v.remaining_loss_rsd != v.recognized_loss_rsd {
            bail!(
                "cannot remove carryforward vintage {vintage_id} for {}: \
                 it has already been partially consumed",
                decl.declaration_id
            );
        }
        self.storage.remove_carryforward_vintage(&vintage_id)
    }

    fn sync_carryforward_vintage(&self, decl: &Declaration, input: &AssessmentInput) -> Result<()> {
        if decl.r#type != DeclarationType::Ppdg3r {
            return Ok(());
        }
        let Some(recognized_loss) = input.recognized_capital_loss_rsd else {
            return Ok(());
        };
        let vintage_id = format!("CF-{}", decl.declaration_id);
        let existing = self.storage.find_carryforward_vintage(&vintage_id);

        if recognized_loss <= Decimal::ZERO {
            self.remove_unconsumed_carryforward_vintage(decl)?;
            return Ok(());
        }

        if let Some(mut v) = existing {
            if v.remaining_loss_rsd != v.recognized_loss_rsd {
                bail!(
                    "cannot update assessment for {}: its carryforward vintage {} \
                     has already been partially consumed",
                    decl.declaration_id,
                    vintage_id
                );
            }
            v.recognized_loss_rsd = recognized_loss;
            v.remaining_loss_rsd = recognized_loss;
            if let Some(r) = &input.assessment_reference {
                v.assessment_reference = Some(r.clone());
            }
            if let Some(n) = &input.assessment_notes {
                v.notes = Some(n.clone());
            }
            self.storage.upsert_carryforward_vintage(v)?;
        } else {
            let vintage = CarryforwardVintage {
                id: vintage_id,
                origin_declaration_id: decl.declaration_id.clone(),
                assessment_reference: input.assessment_reference.clone(),
                origin_period_start: decl.period_start,
                origin_period_end: decl.period_end,
                recognized_loss_rsd: recognized_loss,
                remaining_loss_rsd: recognized_loss,
                created_at: Local::now().naive_local(),
                expiration_tax_year: decl.period_end.year() + 5,
                notes: input.assessment_notes.clone(),
            };
            self.storage.upsert_carryforward_vintage(vintage)?;
        }
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
            self.remove_unconsumed_carryforward_vintage(&decl)?;
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
        tax_due: Option<Decimal>,
        status: DeclarationStatus,
        mark_paid: bool,
    ) -> String {
        match (tax_due, mark_paid) {
            (Some(tax_due), true) => format!("Assessment saved and paid: {id} ({tax_due} RSD)"),
            (Some(_), false) if status == DeclarationStatus::Finalized => {
                format!("Assessment saved: {id} (no tax to pay)")
            }
            (Some(tax_due), false) => format!("Assessment saved: {id} ({tax_due} RSD to pay)"),
            (None, true) => format!("Assessment saved and marked paid: {id}"),
            (None, false) => format!("Assessment saved: {id}"),
        }
    }

    pub fn apply_each<F>(&self, ids: &[&str], mut op: F) -> BulkResult
    where
        F: FnMut(&Self, &str) -> Result<()>,
    {
        let mut ok_count = 0;
        let mut errors = Vec::new();
        for id in ids {
            match op(self, id) {
                Ok(()) => ok_count += 1,
                Err(e) => errors.push(((*id).to_string(), e.to_string())),
            }
        }
        BulkResult { ok_count, errors }
    }

    #[must_use]
    pub fn get_status(&self, id: &str) -> Option<DeclarationStatus> {
        self.storage.get_declaration(id).map(|d| d.status)
    }

    fn get_or_err(&self, id: &str) -> Result<Declaration> {
        self.storage
            .get_declaration(id)
            .ok_or_else(|| anyhow::anyhow!("declaration {id} not found"))
    }
}
