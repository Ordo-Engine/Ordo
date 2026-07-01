use anyhow::Result;
use clap::{Parser, Subcommand};

mod eval;
mod exec;
mod fmt;
mod init;
mod lint;
mod new;
mod output;
mod project;
mod runtime;
mod test_runner;
mod trace;
mod validate;

#[derive(Parser)]
#[command(
    name = "ordo",
    version,
    about = "Ordo — author, test, and ship decision rules"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Emit machine-readable JSON instead of human-readable text
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Evaluate a single expression against input data
    Eval(eval::EvalArgs),
    /// Execute a standalone rule file against input data
    Exec(exec::ExecArgs),
    /// Run a ruleset's test cases
    Test(test_runner::TestArgs),
    /// Execute a project ruleset and show the step-by-step execution path
    Trace(trace::TraceArgs),
    /// Compile a ruleset offline and report any errors
    Validate(validate::ValidateArgs),
    /// Canonically format the project's ruleset files
    Fmt(fmt::FmtArgs),
    /// Lint the project's rulesets for graph and style issues
    Lint(lint::LintArgs),
    /// Scaffold a new decision project in the current directory
    Init(init::InitArgs),
    /// Add a new ruleset, fact, or concept to the project
    New(new::NewArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Eval(args) => eval::run(args, cli.json),
        Commands::Exec(args) => exec::run(args, cli.json),
        Commands::Test(args) => test_runner::run(args, cli.json),
        Commands::Trace(args) => trace::run(args, cli.json),
        Commands::Validate(args) => validate::run(args, cli.json),
        Commands::Fmt(args) => fmt::run(args, cli.json),
        Commands::Lint(args) => lint::run(args, cli.json),
        Commands::Init(args) => init::run(args, cli.json),
        Commands::New(args) => new::run(args, cli.json),
    }
}
