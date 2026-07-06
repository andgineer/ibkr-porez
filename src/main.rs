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
    after_help = "Docs: https://andgineer.github.io/ibkr-porez/en/\n\
                  Run without a command to launch the GUI."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, global = true)]
    verbose: bool,
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
    /// Import transactions from a CSV activity statement (full history, older than 1 year)
    Import {
        /// Path to CSV file (use - or omit for stdin)
        file_path: Option<PathBuf>,
    },
    /// Sync data from IBKR, generate reports and declarations
    Sync {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long, value_parser = clap::value_parser!(i64).range(1..))]
        lookback: Option<i64>,
        /// Import from a locally downloaded Flex Query XML file instead of calling IBKR API
        /// (use - to read from stdin; see <https://andgineer.github.io/ibkr-porez/en/ibkr.html>)
        #[arg(short, long)]
        file: Option<PathBuf>,
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
    Submit {
        /// Declaration ID(s); reads from stdin if omitted
        declaration_id: Vec<String>,
    },
    /// Mark declaration as paid
    Pay {
        /// Declaration ID(s); reads from stdin if omitted
        declaration_id: Vec<String>,
        #[arg(long)]
        tax: Option<Decimal>,
    },
    /// Record a tax authority assessment for a declaration
    Assess {
        declaration_id: String,
        /// Tax amount as determined by the tax authority
        #[arg(short = 't', long)]
        tax: Option<Decimal>,
        /// Capital gain recognized by the tax authority (may differ from calculated)
        #[arg(long)]
        gain: Option<Decimal>,
        /// Capital loss recognized by the tax authority (may differ from calculated)
        #[arg(long)]
        loss: Option<Decimal>,
        /// Reference number of the assessment decision
        #[arg(long)]
        reference: Option<String>,
        /// Date of the assessment decision
        #[arg(long = "date")]
        assessment_date: Option<NaiveDate>,
        /// Free-form notes about the assessment
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        paid: bool,
    },
    /// List recognized capital-loss carryforward vintages
    Carryforward,
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
        /// Declaration ID(s); reads from stdin if omitted
        declaration_id: Vec<String>,
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
    /// Delete an erroneous declaration and regenerate it from stored data
    Regenerate {
        declaration_id: String,
        /// Actually execute (without this flag only prints the plan)
        #[arg(long)]
        yes: bool,
        /// Allow deleting a non-Draft declaration; also passed to report generation
        #[arg(long)]
        force: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let _log_guard = ibkr_porez::logging::init(cli.verbose);

    let result = match cli.command {
        Some(Commands::Config) => cli::config::run(),
        Some(Commands::Fetch) => cli::fetch::run(),
        Some(Commands::Import { file_path }) => cli::import::run(file_path),
        Some(Commands::Sync {
            output,
            lookback,
            file,
        }) => cli::sync::run(output, lookback, file),
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
        Some(Commands::Submit { declaration_id }) => cli::submit::run(declaration_id),
        Some(Commands::Pay {
            declaration_id,
            tax,
        }) => cli::pay::run(declaration_id, tax),
        Some(Commands::Assess {
            declaration_id,
            tax,
            gain,
            loss,
            reference,
            assessment_date,
            notes,
            paid,
        }) => cli::assess::run(
            &declaration_id,
            tax,
            gain,
            loss,
            reference,
            assessment_date,
            notes,
            paid,
        ),
        Some(Commands::Carryforward) => cli::carryforward::run(),
        Some(Commands::Export {
            declaration_id,
            output,
        }) => cli::export::run(&declaration_id, output),
        Some(Commands::ExportFlex { date, output }) => cli::export_flex::run(date, output),
        Some(Commands::Revert { declaration_id, to }) => cli::revert::run(declaration_id, to),
        Some(Commands::Attach {
            declaration_id,
            file_path,
            delete,
            file_id,
        }) => cli::attach::run(&declaration_id, file_path, delete, file_id),
        Some(Commands::Regenerate {
            declaration_id,
            yes,
            force,
        }) => cli::regenerate::run(&declaration_id, yes, force),
        None => launch_gui(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}

fn launch_gui() -> anyhow::Result<()> {
    let exe_path = std::env::current_exe()?;
    let resolved = std::fs::canonicalize(&exe_path).unwrap_or(exe_path);
    let exe_dir = resolved
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();

    let gui_name = if cfg!(windows) {
        "ibkr-porez-gui.exe"
    } else {
        "ibkr-porez-gui"
    };
    let gui_bin = exe_dir.join(gui_name);

    if !gui_bin.exists() {
        eprintln!("GUI binary not found. Run with a subcommand or use --help.");
        process::exit(1);
    }

    eprintln!("Starting GUI...");

    let mut cmd = process::Command::new(&gui_bin);
    cmd.stdin(process::Stdio::null())
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()?;

    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}
