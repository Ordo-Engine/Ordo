use anyhow::Result;
use clap::{Parser, Subcommand};

mod eval;
mod exec;
mod output;
mod runtime;
mod test_runner;

#[derive(Parser)]
#[command(name = "ordo", version, about = "Ordo rule engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Emit machine-readable JSON instead of human-readable text
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Evaluate an expression against input data
    Eval(eval::EvalArgs),
    /// Execute a ruleset against input data
    Exec(exec::ExecArgs),
    /// Run tests against a ruleset
    Test(test_runner::TestArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Eval(args) => eval::run(args, cli.json),
        Commands::Exec(args) => exec::run(args, cli.json),
        Commands::Test(args) => test_runner::run(args, cli.json),
    }
}
