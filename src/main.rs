use std::path::PathBuf;
use std::process;

use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use rust_decimal::Decimal;

mod cli;

#[derive(Parser)]
#[command(
    name = "ibkr-porez",
    version,
    about = "Serbian tax reporting for Interactive Brokers",
    after_help = "Docs: https://andgineer.github.io/ibkr-porez-rs/en/\n\
                  Run without a command to launch the GUI."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Clone, ValueEnum)]
enum ImportType {
    Auto,
    Csv,
    Flex,
}

#[derive(Clone, ValueEnum)]
enum ReportType {
    Gains,
    Income,
}

#[derive(Clone, ValueEnum)]
pub enum RevertTarget {
    Draft,
    Submitted,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure IBKR and personal details
    Config,
    /// Fetch data from IBKR (without generating reports)
    Fetch,
    /// Import transactions from a CSV or Flex XML file
    Import {
        /// Path to file (use - or omit for stdin)
        file_path: Option<PathBuf>,
        #[arg(short = 't', long, default_value = "auto")]
        r#type: ImportType,
    },
    /// Sync data from IBKR, generate reports and declarations
    Sync {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long, value_parser = clap::value_parser!(i64).range(1..))]
        lookback: Option<i64>,
    },
    /// Generate tax reports
    Report {
        #[arg(short = 't', long, default_value = "gains")]
        r#type: ReportType,
        #[arg(long)]
        half: Option<String>,
        #[arg(short = 's', long)]
        start: Option<NaiveDate>,
        #[arg(short = 'e', long)]
        end: Option<NaiveDate>,
        #[arg(long)]
        force: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List declarations
    List {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        status: Option<cli::StatusFilter>,
        #[arg(short = '1', long)]
        ids_only: bool,
    },
    /// Show declaration details
    Show { declaration_id: String },
    /// Show transaction statistics
    Stat {
        #[arg(short = 'y', long)]
        year: Option<i32>,
        #[arg(short = 't', long)]
        ticker: Option<String>,
        #[arg(short = 'm', long)]
        month: Option<String>,
    },
    /// Mark declaration as submitted
    Submit { declaration_id: String },
    /// Mark declaration as paid
    Pay {
        declaration_id: String,
        #[arg(long)]
        tax: Option<Decimal>,
    },
    /// Set assessed tax on a declaration
    Assess {
        declaration_id: String,
        #[arg(short = 't', long)]
        tax_due: Decimal,
        #[arg(long)]
        paid: bool,
    },
    /// Export declaration XML and attachments
    Export {
        declaration_id: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Export flex query XML for a given date
    ExportFlex {
        /// Date in YYYY-MM-DD format
        date: NaiveDate,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Revert declaration to draft or submitted status
    Revert {
        declaration_id: String,
        #[arg(long, default_value = "draft")]
        to: RevertTarget,
    },
    /// Attach or remove a file from a declaration
    Attach {
        declaration_id: String,
        file_path: Option<PathBuf>,
        #[arg(short, long)]
        delete: bool,
        #[arg(long)]
        file_id: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .init();
    }

    let result = match cli.command {
        Some(Commands::Config) => cli::config::run(),
        Some(Commands::Fetch) => cli::fetch::run(),
        Some(Commands::Import { file_path, r#type }) => cli::import::run(file_path, r#type.into()),
        Some(Commands::Sync { output, lookback }) => cli::sync::run(output, lookback),
        Some(Commands::Report {
            r#type,
            half,
            start,
            end,
            force,
            output,
        }) => cli::report::run(r#type.into(), half, start, end, force, output),
        Some(Commands::List {
            all,
            status,
            ids_only,
        }) => cli::list::run(all, status, ids_only),
        Some(Commands::Show { declaration_id }) => cli::show::run(&declaration_id),
        Some(Commands::Stat {
            year,
            ticker,
            month,
        }) => cli::stat::run(year, ticker, month),
        Some(Commands::Submit { declaration_id }) => cli::submit::run(&declaration_id),
        Some(Commands::Pay {
            declaration_id,
            tax,
        }) => cli::pay::run(&declaration_id, tax),
        Some(Commands::Assess {
            declaration_id,
            tax_due,
            paid,
        }) => cli::assess::run(&declaration_id, tax_due, paid),
        Some(Commands::Export {
            declaration_id,
            output,
        }) => cli::export::run(&declaration_id, output),
        Some(Commands::ExportFlex { date, output }) => cli::export_flex::run(date, output),
        Some(Commands::Revert { declaration_id, to }) => cli::revert::run(&declaration_id, to),
        Some(Commands::Attach {
            declaration_id,
            file_path,
            delete,
            file_id,
        }) => cli::attach::run(&declaration_id, file_path, delete, file_id),
        None => launch_gui(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}

fn launch_gui() -> anyhow::Result<()> {
    let gui_bin = std::env::current_exe()?
        .parent()
        .map_or_else(|| PathBuf::from("gui"), |p| p.join("gui"));

    if gui_bin.exists() {
        let status = process::Command::new(&gui_bin).status()?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    } else {
        eprintln!("GUI binary not found. Run with a subcommand or use --help.");
        process::exit(1);
    }
}
