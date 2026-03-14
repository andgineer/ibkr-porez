use anyhow::Result;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Table};
use console::style;

use super::{load_config_or_exit, make_storage, output, tables};
use ibkr_porez::models::{DeclarationType, IncomeDeclarationEntry, TaxReportEntry};

#[allow(clippy::unnecessary_wraps)]
pub fn run(declaration_id: &str) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);

    let Some(decl) = storage.get_declaration(declaration_id) else {
        output::error(&format!("Declaration '{declaration_id}' not found."));
        return Ok(());
    };

    println!(
        "{} {}",
        style("Declaration ID:").bold(),
        decl.declaration_id
    );
    println!("{} {}", style("Type:").bold(), decl.display_type());
    println!("{} {}", style("Status:").bold(), decl.status);
    println!("{} {}", style("Period:").bold(), decl.display_period());
    println!(
        "{} {}",
        style("Created:").bold(),
        decl.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    if let Some(ref dt) = decl.submitted_at {
        println!(
            "{} {}",
            style("Submitted:").bold(),
            dt.format("%Y-%m-%d %H:%M:%S")
        );
    }
    if let Some(ref dt) = decl.paid_at {
        println!(
            "{} {}",
            style("Paid:").bold(),
            dt.format("%Y-%m-%d %H:%M:%S")
        );
    }
    if let Some(ref fp) = decl.file_path {
        println!("{} {fp}", style("File:").bold());
    }

    if let Some(ref data) = decl.report_data {
        if decl.r#type == DeclarationType::Ppdg3r {
            let entries: Vec<TaxReportEntry> = data
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            if !entries.is_empty() {
                println!("\n  Declaration Data (Part 4)");
                println!("{}", tables::render_gains_table(&entries));
            }
        } else {
            let entries: Vec<IncomeDeclarationEntry> = data
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            if !entries.is_empty() {
                println!();
                for entry in &entries {
                    tables::print_income_entry(entry);
                }
            }
        }
    }

    if !decl.metadata.is_empty() {
        print_metadata(&decl.metadata);
    }

    if !decl.attached_files.is_empty() {
        println!("\n{}", style("Attached files:").bold());
        for (name, path) in &decl.attached_files {
            println!("  {}: {}", style(name).cyan(), path);
        }
    }

    Ok(())
}

const METADATA_KEY_ORDER: &[&str] = &[
    "period_start",
    "period_end",
    "entry_count",
    "symbol",
    "income_type",
    "total_gain_rsd",
    "gross_income_rsd",
    "tax_base_rsd",
    "calculated_tax_rsd",
    "estimated_tax_rsd",
    "foreign_tax_paid_rsd",
    "assessed_tax_due_rsd",
    "tax_due_rsd",
];

fn print_metadata(metadata: &indexmap::IndexMap<String, serde_json::Value>) {
    println!();
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    let mut ordered_keys: Vec<&str> = METADATA_KEY_ORDER
        .iter()
        .filter(|k| metadata.contains_key(**k))
        .copied()
        .collect();
    let mut extras: Vec<&str> = metadata
        .keys()
        .map(String::as_str)
        .filter(|k| !ordered_keys.contains(k))
        .collect();
    extras.sort_unstable();
    ordered_keys.extend(extras);

    for key in ordered_keys {
        if let Some(val) = metadata.get(key) {
            let formatted = format_metadata_value(val);
            table.add_row(vec![Cell::new(key).fg(Color::Cyan), Cell::new(formatted)]);
        }
    }
    println!("{table}");
}

fn format_metadata_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                format!("{f:.2}")
            } else {
                n.to_string()
            }
        }
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
