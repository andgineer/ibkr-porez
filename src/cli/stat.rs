use anyhow::Result;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use console::style;

use super::{init_calendar, load_config_or_exit, make_nbs, make_storage, output};
use ibkr_porez::stat::{ShowStatistics, StatMode};

#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
pub fn run(year: Option<i32>, ticker: Option<String>, month: Option<String>) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let cal = init_calendar(&cfg);
    let nbs = make_nbs(&storage, &cal);

    let stats = ShowStatistics::new(&storage, &nbs);
    let result = match stats.generate(year, ticker.as_deref(), month.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("month must be") || msg.contains("invalid month") {
                output::error(&msg);
                return Ok(());
            }
            return Err(e);
        }
    };

    match result.mode {
        StatMode::Empty(msg) => {
            output::warning(&msg);
        }
        StatMode::Aggregated(rows) => {
            println!("{}", style("Monthly Report Breakdown").bold());

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("Month").fg(Color::Cyan),
                    Cell::new("Ticker").fg(Color::Cyan),
                    Cell::new("Dividends (RSD)").fg(Color::Cyan),
                    Cell::new("Sales Count").fg(Color::Cyan),
                    Cell::new("Realized P/L (RSD)").fg(Color::Cyan),
                ]);

            let mut prev_month: Option<String> = None;
            for row in &rows {
                if prev_month.as_ref().is_some_and(|pm| pm != &row.month) {
                    table.add_row(vec![
                        Cell::new(""),
                        Cell::new(""),
                        Cell::new(""),
                        Cell::new(""),
                        Cell::new(""),
                    ]);
                }
                prev_month = Some(row.month.clone());

                table.add_row(vec![
                    Cell::new(&row.month),
                    Cell::new(&row.ticker),
                    Cell::new(output::format_thousands_2f(row.dividends_rsd)),
                    Cell::new(row.sales_count),
                    Cell::new(output::format_thousands_2f(row.realized_pnl_rsd)),
                ]);
            }

            println!("{table}");
        }
        StatMode::Detailed {
            rows,
            total_pnl,
            title,
        } => {
            println!("{}\n", style(&title).bold());
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec![
                    Cell::new("Sale Date").fg(Color::Cyan),
                    Cell::new("Qty").fg(Color::Cyan),
                    Cell::new("Sale Price").fg(Color::Cyan),
                    Cell::new("Sale Rate").fg(Color::Cyan),
                    Cell::new("Sale Val (RSD)").fg(Color::Cyan),
                    Cell::new("Buy Date").fg(Color::Cyan),
                    Cell::new("Buy Price").fg(Color::Cyan),
                    Cell::new("Buy Rate").fg(Color::Cyan),
                    Cell::new("Buy Val (RSD)").fg(Color::Cyan),
                    Cell::new("Gain (RSD)").fg(Color::Cyan),
                ]);

            for row in &rows {
                table.add_row(vec![
                    Cell::new(&row.sale_date),
                    Cell::new(format!("{:.2}", row.quantity)),
                    Cell::new(format!("{:.2}", row.sale_price)),
                    Cell::new(format!("{:.4}", row.sale_rate)),
                    Cell::new(output::format_thousands_0f(row.sale_value_rsd)),
                    Cell::new(&row.buy_date),
                    Cell::new(format!("{:.2}", row.buy_price)),
                    Cell::new(format!("{:.4}", row.buy_rate)),
                    Cell::new(output::format_thousands_0f(row.buy_value_rsd)),
                    Cell::new(output::format_thousands_2f(row.gain_rsd))
                        .add_attribute(Attribute::Bold),
                ]);
            }

            println!("{table}");
            output::bold(&format!(
                "Total P/L: {} RSD",
                output::format_thousands_2f(total_pnl)
            ));
        }
    }

    Ok(())
}
