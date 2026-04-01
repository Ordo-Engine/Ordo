use anyhow::Result;
use clap::{Parser, Subcommand};

mod eval;
mod exec;
mod test_runner;

#[derive(Parser)]
#[command(name = "ordo", version, about = "Ordo rule engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
        Commands::Eval(args) => eval::run(args),
        Commands::Exec(args) => exec::run(args),
        Commands::Test(args) => test_runner::run(args),
    }
}
