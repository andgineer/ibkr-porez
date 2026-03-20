use anyhow::Result;
use console::style;
use dialoguer::Input;

use ibkr_porez::config as app_config;
use ibkr_porez::models::UserConfig;

use super::output;

struct FieldDef {
    label: &'static str,
    struct_field: &'static str,
    hint: &'static str,
}

const FIELDS: &[FieldDef] = &[
    FieldDef {
        label: "IBKR Flex Token",
        struct_field: "ibkr_token",
        hint: "(see https://andgineer.github.io/ibkr-porez/en/ibkr.html)",
    },
    FieldDef {
        label: "IBKR Flex Query ID",
        struct_field: "ibkr_query_id",
        hint: "(see https://andgineer.github.io/ibkr-porez/en/ibkr.html)",
    },
    FieldDef {
        label: "Personal ID (JMBG)",
        struct_field: "personal_id",
        hint: "",
    },
    FieldDef {
        label: "Full Name",
        struct_field: "full_name",
        hint: "",
    },
    FieldDef {
        label: "Address",
        struct_field: "address",
        hint: "",
    },
    FieldDef {
        label: "City/Municipality Code",
        struct_field: "city_code",
        hint: "(see https://andgineer.github.io/ibkr-porez/en/usage.html)",
    },
    FieldDef {
        label: "Phone",
        struct_field: "phone",
        hint: "",
    },
    FieldDef {
        label: "Email",
        struct_field: "email",
        hint: "",
    },
    FieldDef {
        label: "Data Directory",
        struct_field: "",
        hint: "",
    },
    FieldDef {
        label: "Output Folder",
        struct_field: "",
        hint: "",
    },
];

pub fn run() -> Result<()> {
    let old_cfg = app_config::load_config();
    let config_path = app_config::config_file_path();

    output::info("Configuration");
    println!("Config file: {}", style(config_path.display()).dim());
    println!(
        "Docs: {}\n",
        style("https://andgineer.github.io/ibkr-porez/en/usage.html").cyan()
    );

    if is_config_empty(&old_cfg) {
        println!("{}", style("Initial Configuration Setup").bold());
        println!("All fields need to be configured.\n");
        let new_cfg = prompt_all_fields(&old_cfg)?;
        save_and_report(&old_cfg, &new_cfg)?;
        return Ok(());
    }

    display_current_values(&old_cfg);
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
            && n <= FIELDS.len()
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
    let issues = app_config::validate_config(cfg);
    for (i, (fd, val)) in FIELDS.iter().zip(values.iter()).enumerate() {
        let val_display = if val.is_empty() {
            style("(not set)").dim().to_string()
        } else {
            val.clone()
        };
        let warning = issues
            .iter()
            .find(|issue| !fd.struct_field.is_empty() && issue.field == fd.struct_field)
            .map(|issue| format!("  {}", style(format!("⚠ {}", issue.message)).yellow()));
        let suffix = warning
            .as_deref()
            .or(if fd.hint.is_empty() {
                None
            } else {
                Some(fd.hint)
            })
            .unwrap_or("");
        if suffix.is_empty() {
            println!("  {:>2}. {}: {val_display}", i + 1, style(fd.label).cyan());
        } else {
            println!(
                "  {:>2}. {}: {val_display}  {}",
                i + 1,
                style(fd.label).cyan(),
                if warning.is_some() {
                    suffix.to_string()
                } else {
                    style(suffix).dim().to_string()
                },
            );
        }
    }
}

fn field_values(cfg: &UserConfig) -> Vec<String> {
    vec![
        cfg.ibkr_token.clone(),
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
    let ibkr_token = prompt_text(
        "IBKR Flex Token (see https://andgineer.github.io/ibkr-porez/en/ibkr.html)",
        &old.ibkr_token,
    )?;

    let ibkr_query_id = prompt_text(
        "IBKR Flex Query ID (see https://andgineer.github.io/ibkr-porez/en/ibkr.html)",
        &old.ibkr_query_id,
    )?;

    let personal_id = prompt_text("Personal ID (JMBG)", &old.personal_id)?;
    let full_name = prompt_text("Full Name (as registered)", &old.full_name)?;
    let address = prompt_text("Address", &old.address)?;
    let city_code = prompt_text(
        "City/Municipality Code (see https://andgineer.github.io/ibkr-porez/en/usage.html)",
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
        0 => {
            cfg.ibkr_token = prompt_text(
                "IBKR Flex Token (see https://andgineer.github.io/ibkr-porez/en/ibkr.html)",
                &cfg.ibkr_token,
            )?;
        }
        1 => {
            cfg.ibkr_query_id = prompt_text(
                "IBKR Flex Query ID (see https://andgineer.github.io/ibkr-porez/en/ibkr.html)",
                &cfg.ibkr_query_id,
            )?;
        }
        2 => cfg.personal_id = prompt_text("Personal ID (JMBG)", &cfg.personal_id)?,
        3 => cfg.full_name = prompt_text("Full Name", &cfg.full_name)?,
        4 => cfg.address = prompt_text("Address", &cfg.address)?,
        5 => {
            cfg.city_code = prompt_text(
                "City/Municipality Code (see https://andgineer.github.io/ibkr-porez/en/usage.html)",
                &cfg.city_code,
            )?;
        }
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

    let issues = app_config::validate_config(new_cfg);
    for issue in &issues {
        output::warning(&format!("Warning: {} — {}", issue.label, issue.message));
    }
    Ok(())
}

fn prompt_text(prompt: &str, default: &str) -> Result<String> {
    let val: String = Input::new()
        .with_prompt(prompt)
        .default(default.to_string())
        .interact_text()?;
    Ok(val)
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
