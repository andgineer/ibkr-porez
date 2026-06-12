use chrono::{Datelike, Local};
use eframe::egui;

use crate::models::{CarryforwardVintage, sort_oldest_origin_first};
use crate::storage::Storage;

use super::app::App;

pub struct CarryforwardDialog {
    pub text: String,
}

impl CarryforwardDialog {
    pub fn new(storage: &Storage) -> Self {
        let mut vintages = storage.get_carryforward_vintages();
        let current_year = Local::now().year();
        Self {
            text: format_carryforward(&mut vintages, current_year),
        }
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    let Some(ref _dialog) = app.carryforward_dialog else {
        return;
    };

    let mut dismiss = false;

    egui::Window::new("Capital Loss Carryforward")
        .collapsible(false)
        .resizable(true)
        .default_width(600.0)
        .default_height(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let dialog = app.carryforward_dialog.as_ref().unwrap();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut dialog.text.as_str())
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                dismiss = true;
            }
        });

    if dismiss {
        app.carryforward_dialog = None;
    }
}

fn format_carryforward(vintages: &mut [CarryforwardVintage], current_year: i32) -> String {
    if vintages.is_empty() {
        return "No carryforward vintages recorded.".to_string();
    }

    sort_oldest_origin_first(vintages);

    let mut lines = Vec::new();
    lines.push(format!(
        "{:<8} {:<24} {:>12} {:>12} {:>8}  {}",
        "ID", "Origin Period", "Recognized", "Remaining", "Expires", "Status"
    ));
    for v in vintages {
        let period = format!(
            "{} - {}",
            v.origin_period_start.format("%Y-%m-%d"),
            v.origin_period_end.format("%Y-%m-%d"),
        );
        lines.push(format!(
            "{:<8} {:<24} {:>12.2} {:>12.2} {:>8}  {}",
            v.id,
            period,
            v.recognized_loss_rsd,
            v.remaining_loss_rsd,
            v.expiration_tax_year,
            v.status(current_year)
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn make_vintage(
        id: &str,
        origin_end: &str,
        recognized: Decimal,
        remaining: Decimal,
        expiration_tax_year: i32,
        created_at: &str,
    ) -> CarryforwardVintage {
        CarryforwardVintage {
            id: id.to_string(),
            origin_declaration_id: "1".to_string(),
            assessment_reference: None,
            origin_period_start: NaiveDate::parse_from_str("2024-01-01", "%Y-%m-%d").unwrap(),
            origin_period_end: NaiveDate::parse_from_str(origin_end, "%Y-%m-%d").unwrap(),
            recognized_loss_rsd: recognized,
            remaining_loss_rsd: remaining,
            created_at: chrono::NaiveDateTime::parse_from_str(
                &format!("{created_at} 00:00:00"),
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
            expiration_tax_year,
            notes: None,
        }
    }

    #[test]
    fn empty_vintages_shows_message() {
        assert_eq!(
            format_carryforward(&mut [], 2025),
            "No carryforward vintages recorded."
        );
    }

    #[test]
    fn active_vintage_shows_status_and_amounts() {
        let mut v = vec![make_vintage(
            "CF-3",
            "2024-06-30",
            dec!(50000),
            dec!(20000),
            2029,
            "2024-08-01",
        )];
        let text = format_carryforward(&mut v, 2025);
        assert!(text.contains("CF-3"));
        assert!(text.contains("2024-01-01 - 2024-06-30"));
        assert!(text.contains("50000.00"));
        assert!(text.contains("20000.00"));
        assert!(text.contains("2029"));
        assert!(text.contains("Active"));
    }

    #[test]
    fn exhausted_vintage_with_zero_remaining() {
        let mut v = vec![make_vintage(
            "CF-1",
            "2022-06-30",
            dec!(15000),
            dec!(0),
            2027,
            "2022-08-01",
        )];
        let text = format_carryforward(&mut v, 2025);
        assert!(text.contains("Exhausted"));
    }

    #[test]
    fn expired_vintage_even_with_remaining_balance() {
        let mut v = vec![make_vintage(
            "CF-0",
            "2019-06-30",
            dec!(10000),
            dec!(5000),
            2024,
            "2019-08-01",
        )];
        let text = format_carryforward(&mut v, 2025);
        assert!(text.contains("Expired"));
    }

    #[test]
    fn sorted_oldest_origin_period_first() {
        let mut v = vec![
            make_vintage(
                "CF-3",
                "2024-06-30",
                dec!(50000),
                dec!(20000),
                2029,
                "2024-08-01",
            ),
            make_vintage(
                "CF-1",
                "2022-06-30",
                dec!(15000),
                dec!(0),
                2027,
                "2022-08-01",
            ),
        ];
        let text = format_carryforward(&mut v, 2025);
        let pos_cf1 = text.find("CF-1").unwrap();
        let pos_cf3 = text.find("CF-3").unwrap();
        assert!(pos_cf1 < pos_cf3);
    }
}
