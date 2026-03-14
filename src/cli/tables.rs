use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use rust_decimal::Decimal;

use ibkr_porez::models::{IncomeDeclarationEntry, TaxReportEntry};

pub fn render_gains_table(entries: &[TaxReportEntry]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("No.").fg(Color::Cyan),
            Cell::new("Ticker (Naziv)").fg(Color::Cyan),
            Cell::new("Sale Date (4.3)").fg(Color::Cyan),
            Cell::new("Qty (4.5/4.9)").fg(Color::Cyan),
            Cell::new("Sale Price RSD (4.6)").fg(Color::Cyan),
            Cell::new("Buy Date (4.7)").fg(Color::Cyan),
            Cell::new("Buy Price RSD (4.10)").fg(Color::Cyan),
            Cell::new("Gain RSD").fg(Color::Cyan),
            Cell::new("Loss RSD").fg(Color::Cyan),
        ]);

    for (i, entry) in entries.iter().enumerate() {
        let (gain, loss) = if entry.capital_gain_rsd >= Decimal::ZERO {
            (format!("{:.2}", entry.capital_gain_rsd), String::new())
        } else {
            (
                String::new(),
                format!("{:.2}", entry.capital_gain_rsd.abs()),
            )
        };

        table.add_row(vec![
            Cell::new(i + 1),
            Cell::new(&entry.ticker),
            Cell::new(entry.sale_date.format("%Y-%m-%d")),
            Cell::new(format!("{:.4}", entry.quantity)),
            Cell::new(format!("{:.2}", entry.sale_value_rsd)),
            Cell::new(entry.purchase_date.format("%Y-%m-%d")),
            Cell::new(format!("{:.2}", entry.purchase_value_rsd)),
            Cell::new(&gain).fg(Color::Green),
            Cell::new(&loss).fg(Color::Red),
        ]);
    }

    table
}

pub fn print_income_entry(entry: &IncomeDeclarationEntry) {
    use console::style;

    println!(
        "  {} {} | {} | Bruto: {:.2} | Osnovica: {:.2} | Porez: {:.2} | Placen: {:.2} | Za uplatu: {:.2}",
        style(entry.date.format("%Y-%m-%d")).cyan(),
        entry.symbol_or_currency.as_deref().unwrap_or("-"),
        style(&entry.sifra_vrste_prihoda).magenta(),
        entry.bruto_prihod,
        entry.osnovica_za_porez,
        entry.obracunati_porez,
        entry.porez_placen_drugoj_drzavi,
        entry.porez_za_uplatu,
    );
}

pub fn render_declarations_table(declarations: &[ibkr_porez::models::Declaration]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").fg(Color::Cyan),
            Cell::new("Type").fg(Color::Cyan),
            Cell::new("Period").fg(Color::Cyan),
            Cell::new("Tax RSD"),
            Cell::new("Status").fg(Color::Cyan),
            Cell::new("Created").fg(Color::Cyan),
            Cell::new("Attachments").fg(Color::Cyan),
        ]);

    for decl in declarations {
        let type_str = match decl.r#type {
            ibkr_porez::models::DeclarationType::Ppdg3r => "PPDG-3R",
            ibkr_porez::models::DeclarationType::Ppo => "PP OPO",
        };
        let period = format!(
            "{} - {}",
            decl.period_start.format("%Y-%m-%d"),
            decl.period_end.format("%Y-%m-%d"),
        );
        let tax = tax_from_metadata(decl);
        let status_str = decl.status.to_string();
        let created = decl.created_at.format("%Y-%m-%d %H:%M").to_string();
        let attachments = if decl.attached_files.is_empty() {
            String::new()
        } else {
            format!("{} attachments", decl.attached_files.len())
        };

        table.add_row(vec![
            Cell::new(&decl.declaration_id).fg(Color::Cyan),
            Cell::new(type_str).fg(Color::Magenta),
            Cell::new(&period).fg(Color::Green),
            Cell::new(&tax),
            Cell::new(&status_str).fg(Color::Yellow),
            Cell::new(&created).fg(Color::Blue),
            Cell::new(&attachments).add_attribute(Attribute::Dim),
        ]);
    }

    table
}

fn tax_from_metadata(decl: &ibkr_porez::models::Declaration) -> String {
    if let Some(v) = decl
        .metadata
        .get("assessed_tax_due_rsd")
        .and_then(|v| v.as_str())
    {
        return v.to_string();
    }
    if let Some(v) = decl.metadata.get("tax_due_rsd").and_then(|v| v.as_str()) {
        return v.to_string();
    }
    String::new()
}
