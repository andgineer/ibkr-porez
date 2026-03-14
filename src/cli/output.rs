use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use rust_decimal::Decimal;

pub fn success(msg: &str) {
    println!("{}", style(msg).green());
}

pub fn warning(msg: &str) {
    println!("{}", style(msg).yellow());
}

pub fn error(msg: &str) {
    eprintln!("{}", style(msg).red());
}

pub fn attention(msg: &str) {
    println!("{}", style(msg).red().bold());
}

pub fn info(msg: &str) {
    println!("{}", style(msg).blue());
}

pub fn bold(msg: &str) {
    println!("{}", style(msg).bold());
}

pub fn bold_success(msg: &str) {
    println!("{}", style(msg).green().bold());
}

pub fn dim(msg: &str) {
    println!("{}", style(msg).dim());
}

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .expect("valid template"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

pub fn format_thousands_2f(val: Decimal) -> String {
    let s = format!("{val:.2}");
    insert_thousands_separator(&s)
}

pub fn format_thousands_0f(val: Decimal) -> String {
    let rounded = val.round_dp(0);
    let s = format!("{rounded}");
    insert_thousands_separator(&s)
}

fn insert_thousands_separator(s: &str) -> String {
    let (integer, decimal) = match s.find('.') {
        Some(dot) => (&s[..dot], Some(&s[dot..])),
        None => (s, None),
    };

    let negative = integer.starts_with('-');
    let digits = if negative { &integer[1..] } else { integer };

    let mut result = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    let mut out = result.chars().rev().collect::<String>();
    if negative {
        out.insert(0, '-');
    }
    if let Some(dec) = decimal {
        out.push_str(dec);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_format_thousands_2f() {
        assert_eq!(format_thousands_2f(dec!(1234.56)), "1,234.56");
        assert_eq!(format_thousands_2f(dec!(0)), "0.00");
        assert_eq!(format_thousands_2f(dec!(999)), "999.00");
        assert_eq!(format_thousands_2f(dec!(1000000.1)), "1,000,000.10");
        assert_eq!(format_thousands_2f(dec!(-1234.5)), "-1,234.50");
    }

    #[test]
    fn test_format_thousands_0f() {
        assert_eq!(format_thousands_0f(dec!(1234)), "1,234");
        assert_eq!(format_thousands_0f(dec!(0)), "0");
        assert_eq!(format_thousands_0f(dec!(999.9)), "1,000");
    }
}
