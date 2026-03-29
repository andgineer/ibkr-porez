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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use ibkr_porez::models::{Declaration, DeclarationStatus, DeclarationType};
    use indexmap::IndexMap;
    use rust_decimal_macros::dec;

    fn make_gains_entry(ticker: &str, gain: Decimal) -> TaxReportEntry {
        TaxReportEntry {
            ticker: ticker.to_string(),
            quantity: dec!(10),
            sale_date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
            sale_price: dec!(170),
            sale_exchange_rate: dec!(108),
            sale_value_rsd: dec!(18360),
            purchase_date: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            purchase_price: dec!(150),
            purchase_exchange_rate: dec!(107),
            purchase_value_rsd: dec!(16050),
            capital_gain_rsd: gain,
            is_tax_exempt: false,
        }
    }

    #[test]
    fn render_gains_table_contains_ticker() {
        let entries = vec![make_gains_entry("AAPL", dec!(2310))];
        let table = render_gains_table(&entries);
        let output = table.to_string();
        assert!(output.contains("AAPL"));
        assert!(output.contains("2310"));
    }

    #[test]
    fn render_gains_table_shows_loss() {
        let entries = vec![make_gains_entry("TSLA", dec!(-500))];
        let table = render_gains_table(&entries);
        let output = table.to_string();
        assert!(output.contains("TSLA"));
        assert!(output.contains("500.00"));
    }

    #[test]
    fn render_gains_table_empty() {
        let entries: Vec<TaxReportEntry> = vec![];
        let table = render_gains_table(&entries);
        let output = table.to_string();
        assert!(output.contains("Ticker"));
    }

    #[test]
    fn print_income_entry_does_not_panic() {
        let entry = IncomeDeclarationEntry {
            date: NaiveDate::from_ymd_opt(2025, 4, 14).unwrap(),
            symbol_or_currency: Some("AAPL".into()),
            sifra_vrste_prihoda: "111402000".into(),
            bruto_prihod: dec!(2700),
            osnovica_za_porez: dec!(2700),
            obracunati_porez: dec!(405),
            porez_placen_drugoj_drzavi: dec!(405),
            porez_za_uplatu: dec!(0),
        };
        // Writes to stdout; we verify it doesn't panic and produces output
        print_income_entry(&entry);
    }

    #[test]
    fn print_income_entry_none_symbol() {
        let entry = IncomeDeclarationEntry {
            date: NaiveDate::from_ymd_opt(2025, 4, 14).unwrap(),
            symbol_or_currency: None,
            sifra_vrste_prihoda: "111402000".into(),
            bruto_prihod: dec!(100),
            osnovica_za_porez: dec!(100),
            obracunati_porez: dec!(15),
            porez_placen_drugoj_drzavi: dec!(15),
            porez_za_uplatu: dec!(0),
        };
        print_income_entry(&entry);
    }

    fn make_test_decl(metadata: IndexMap<String, serde_json::Value>) -> Declaration {
        Declaration {
            declaration_id: "t".into(),
            r#type: DeclarationType::Ppo,
            status: DeclarationStatus::Draft,
            period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
            created_at: chrono::NaiveDateTime::default(),
            submitted_at: None,
            paid_at: None,
            file_path: None,
            xml_content: None,
            report_data: None,
            metadata,
            attached_files: IndexMap::new(),
        }
    }

    #[test]
    fn render_declarations_table_with_data() {
        let mut metadata = IndexMap::new();
        metadata.insert(
            "tax_due_rsd".into(),
            serde_json::Value::String("1000.00".into()),
        );

        let mut decl = make_test_decl(metadata);
        decl.declaration_id = "decl-1".into();
        decl.r#type = DeclarationType::Ppdg3r;
        decl.created_at = chrono::NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
            chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
        );

        let table = render_declarations_table(&[decl]);
        let output = table.to_string();
        assert!(output.contains("decl-1"));
        assert!(output.contains("PPDG-3R"));
        assert!(output.contains("1000.00"));
    }

    #[test]
    fn tax_from_metadata_prefers_assessed() {
        let mut metadata = IndexMap::new();
        metadata.insert(
            "tax_due_rsd".into(),
            serde_json::Value::String("500".into()),
        );
        metadata.insert(
            "assessed_tax_due_rsd".into(),
            serde_json::Value::String("750".into()),
        );
        assert_eq!(super::tax_from_metadata(&make_test_decl(metadata)), "750");
    }

    #[test]
    fn tax_from_metadata_falls_back_to_tax_due() {
        let mut metadata = IndexMap::new();
        metadata.insert(
            "tax_due_rsd".into(),
            serde_json::Value::String("500".into()),
        );
        assert_eq!(super::tax_from_metadata(&make_test_decl(metadata)), "500");
    }

    #[test]
    fn tax_from_metadata_empty_when_no_keys() {
        assert!(super::tax_from_metadata(&make_test_decl(IndexMap::new())).is_empty());
    }
}
