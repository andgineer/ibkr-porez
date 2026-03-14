use anyhow::Result;
use console::style;
use dialoguer::Input;

use ibkr_porez::config as app_config;
use ibkr_porez::models::UserConfig;

use super::output;

const FIELD_NAMES: &[&str] = &[
    "IBKR Flex Token",
    "IBKR Flex Query ID",
    "Personal ID (JMBG)",
    "Full Name",
    "Address",
    "City/Municipality Code",
    "Phone",
    "Email",
    "Data Directory",
    "Output Folder",
];

pub fn run() -> Result<()> {
    let old_cfg = app_config::load_config();
    let config_path = app_config::config_file_path();

    output::info("Configuration");
    println!("Config file: {}\n", style(config_path.display()).dim());

    if is_config_empty(&old_cfg) {
        println!("{}", style("Initial Configuration Setup").bold());
        println!("All fields need to be configured.\n");
        let new_cfg = prompt_all_fields(&old_cfg)?;
        save_and_report(&old_cfg, &new_cfg)?;
        return Ok(());
    }

    display_current_values(&old_cfg);

    println!();
    for (i, name) in FIELD_NAMES.iter().enumerate() {
        println!("  {:>2}. {name}", i + 1);
    }
    println!("   A. Update all fields");
    println!("   Q. Done (save & exit)");
    println!();

    let input: String = Input::new()
        .with_prompt("Select fields to update (comma-separated, 'all', or Enter to skip)")
        .default("Q".into())
        .interact_text()?;

    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("q") {
        output::success("No changes made.");
    } else if trimmed.eq_ignore_ascii_case("a") || trimmed.eq_ignore_ascii_case("all") {
        let new_cfg = prompt_all_fields(&old_cfg)?;
        save_and_report(&old_cfg, &new_cfg)?;
    } else {
        let indices = parse_field_indices(trimmed);
        if indices.is_empty() {
            output::warning("No valid field numbers entered.");
        } else {
            let mut cfg = old_cfg.clone();
            for idx in indices {
                prompt_single_field(&mut cfg, idx)?;
            }
            save_and_report(&old_cfg, &cfg)?;
        }
    }
    Ok(())
}

fn parse_field_indices(input: &str) -> Vec<usize> {
    let mut indices = Vec::new();
    for part in input.split(',') {
        if let Ok(n) = part.trim().parse::<usize>()
            && n >= 1
            && n <= FIELD_NAMES.len()
            && !indices.contains(&(n - 1))
        {
            indices.push(n - 1);
        }
    }
    indices
}

fn is_config_empty(cfg: &UserConfig) -> bool {
    cfg.ibkr_token.is_empty()
        && cfg.ibkr_query_id.is_empty()
        && cfg.full_name.is_empty()
        && cfg.address.is_empty()
}

fn display_current_values(cfg: &UserConfig) {
    println!("{}", style("Current Configuration:").bold());
    let values = field_values(cfg);
    for (i, (name, val)) in FIELD_NAMES.iter().zip(values.iter()).enumerate() {
        println!(
            "  {:>2}. {}: {}",
            i + 1,
            style(name).cyan(),
            if val.is_empty() {
                style("(not set)").dim().to_string()
            } else {
                val.clone()
            }
        );
    }
}

fn field_values(cfg: &UserConfig) -> Vec<String> {
    vec![
        mask(&cfg.ibkr_token),
        cfg.ibkr_query_id.clone(),
        cfg.personal_id.clone(),
        cfg.full_name.clone(),
        cfg.address.clone(),
        cfg.city_code.clone(),
        cfg.phone.clone(),
        cfg.email.clone(),
        cfg.data_dir.clone().unwrap_or_default(),
        cfg.output_folder.clone().unwrap_or_default(),
    ]
}

fn prompt_all_fields(old: &UserConfig) -> Result<UserConfig> {
    let ibkr_token = prompt_token(
        "IBKR Flex Token (see https://www.interactivebrokers.com/en/software/am3/am/manageaccount/deliverflexquerybyweb.htm)",
        &old.ibkr_token,
    )?;

    let ibkr_query_id = prompt_text(
        "IBKR Flex Query ID (see https://www.interactivebrokers.com/en/software/am3/am/manageaccount/deliverflexquerybyweb.htm)",
        &old.ibkr_query_id,
    )?;

    let personal_id = prompt_text("Personal ID (JMBG)", &old.personal_id)?;
    let full_name = prompt_text("Full Name (as registered)", &old.full_name)?;
    let address = prompt_text("Address", &old.address)?;
    let city_code = prompt_text(
        "City/Municipality Code (Sifra opstine, e.g. 223 Novi Sad, 013 Novi Beograd. See portal)",
        &old.city_code,
    )?;
    let phone = prompt_text("Phone", &old.phone)?;
    let email = prompt_text("Email", &old.email)?;
    let data_dir = prompt_optional(
        "Data Directory (leave empty for default)",
        old.data_dir.as_ref(),
    )?;
    let output_folder = prompt_optional(
        "Output Folder (leave empty for ~/Downloads)",
        old.output_folder.as_ref(),
    )?;

    Ok(UserConfig {
        ibkr_token,
        ibkr_query_id,
        personal_id,
        full_name,
        address,
        city_code,
        phone,
        email,
        data_dir,
        output_folder,
    })
}

fn prompt_single_field(cfg: &mut UserConfig, idx: usize) -> Result<()> {
    match idx {
        0 => cfg.ibkr_token = prompt_token("IBKR Flex Token", &cfg.ibkr_token)?,
        1 => cfg.ibkr_query_id = prompt_text("IBKR Flex Query ID", &cfg.ibkr_query_id)?,
        2 => cfg.personal_id = prompt_text("Personal ID (JMBG)", &cfg.personal_id)?,
        3 => cfg.full_name = prompt_text("Full Name", &cfg.full_name)?,
        4 => cfg.address = prompt_text("Address", &cfg.address)?,
        5 => cfg.city_code = prompt_text("City/Municipality Code", &cfg.city_code)?,
        6 => cfg.phone = prompt_text("Phone", &cfg.phone)?,
        7 => cfg.email = prompt_text("Email", &cfg.email)?,
        8 => cfg.data_dir = prompt_optional("Data Directory", cfg.data_dir.as_ref())?,
        9 => cfg.output_folder = prompt_optional("Output Folder", cfg.output_folder.as_ref())?,
        _ => {}
    }
    Ok(())
}

fn save_and_report(old_cfg: &UserConfig, new_cfg: &UserConfig) -> Result<()> {
    if let Some(warning) = app_config::get_data_dir_change_warning(old_cfg, new_cfg) {
        output::warning(&format!("Warning: {warning}"));
    }

    app_config::save_config(new_cfg)?;
    output::success("Configuration saved successfully!");
    Ok(())
}

fn prompt_text(prompt: &str, default: &str) -> Result<String> {
    let val: String = Input::new()
        .with_prompt(prompt)
        .default(default.to_string())
        .interact_text()?;
    Ok(val)
}

fn prompt_token(prompt: &str, current: &str) -> Result<String> {
    let hint = mask(current);
    let val: String = Input::new()
        .with_prompt(format!("{prompt} [{hint}] (Enter to keep)"))
        .default(String::new())
        .allow_empty(true)
        .interact_text()?;
    if val.is_empty() {
        Ok(current.to_string())
    } else {
        Ok(val)
    }
}

fn prompt_optional(prompt: &str, current: Option<&String>) -> Result<Option<String>> {
    let val: String = Input::new()
        .with_prompt(prompt)
        .default(current.cloned().unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;
    if val.is_empty() {
        Ok(None)
    } else {
        Ok(Some(val))
    }
}

fn mask(s: &str) -> String {
    if s.is_empty() {
        return "(not set)".into();
    }
    if s.len() <= 4 {
        return "****".into();
    }
    format!("{}…{}", &s[..2], &s[s.len() - 2..])
}
