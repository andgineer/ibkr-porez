use clap::Parser;

#[derive(Parser)]
#[command(
    name = "ibkr-porez",
    version,
    about = "Serbian tax reporting for Interactive Brokers",
    after_help = "Run without a command to launch the GUI."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Configure IBKR and personal details
    Config,
    /// Sync data from IBKR and NBS
    Fetch,
    /// Generate tax reports
    Report,
    /// List declarations
    List,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config) => println!("config: not implemented yet"),
        Some(Commands::Fetch) => println!("fetch: not implemented yet"),
        Some(Commands::Report) => println!("report: not implemented yet"),
        Some(Commands::List) => println!("list: not implemented yet"),
        None => println!("gui: not implemented yet"),
    }
}
