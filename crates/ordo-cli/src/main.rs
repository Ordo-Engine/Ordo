use anyhow::Result;
use clap::{Parser, Subcommand};

mod api;
mod config;
mod deployments;
mod diff;
mod eval;
mod exec;
mod fmt;
mod init;
mod link;
mod lint;
mod login;
mod mcp;
mod new;
mod output;
mod project;
mod publish;
mod pull;
mod push;
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
    // ── local dev loop ──
    /// Scaffold a new decision project in the current directory
    Init(init::InitArgs),
    /// Add a new ruleset, fact, or concept to the project
    New(new::NewArgs),
    /// Evaluate a single expression against input data
    Eval(eval::EvalArgs),
    /// Execute a standalone rule file against input data
    Exec(exec::ExecArgs),
    /// Compile a ruleset offline and report any errors
    Validate(validate::ValidateArgs),
    /// Execute a project ruleset and show the step-by-step execution path
    Trace(trace::TraceArgs),
    /// Run a ruleset's test cases
    Test(test_runner::TestArgs),
    /// Canonically format the project's ruleset files
    Fmt(fmt::FmtArgs),
    /// Lint the project's rulesets for graph and style issues
    Lint(lint::LintArgs),

    // ── platform (remote) ──
    /// Log in to the platform
    Login(login::LoginArgs),
    /// Show the current authenticated user
    Whoami,
    /// Link this project to a platform organization + project
    Link(link::LinkArgs),
    /// Pull rulesets + catalog from the platform into local files
    Pull(pull::PullArgs),
    /// Push local rulesets to the platform
    Push(push::PushArgs),
    /// Publish a ruleset to an environment
    Publish(publish::PublishArgs),
    /// List recent deployments
    Deployments(deployments::DeploymentsArgs),
    /// Compare local rulesets against the platform's drafts
    Diff(diff::DiffArgs),

    /// Run Ordo as an MCP server (stdio) — exposes tools to a coding agent
    Mcp(mcp::McpArgs),
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let json = cli.json;
    match cli.command {
        // local (synchronous — no network)
        Commands::Init(a) => init::run(a, json),
        Commands::New(a) => new::run(a, json),
        Commands::Eval(a) => eval::run(a, json),
        Commands::Exec(a) => exec::run(a, json),
        Commands::Validate(a) => validate::run(a, json),
        Commands::Trace(a) => trace::run(a, json),
        Commands::Test(a) => test_runner::run(a, json),
        Commands::Fmt(a) => fmt::run(a, json),
        Commands::Lint(a) => lint::run(a, json),
        // platform (async)
        Commands::Login(a) => login::run(a, json).await,
        Commands::Whoami => login::whoami(json).await,
        Commands::Link(a) => link::run(a, json).await,
        Commands::Pull(a) => pull::run(a, json).await,
        Commands::Push(a) => push::run(a, json).await,
        Commands::Publish(a) => publish::run(a, json).await,
        Commands::Deployments(a) => deployments::run(a, json).await,
        Commands::Diff(a) => diff::run(a, json).await,
        Commands::Mcp(a) => {
            mcp::run(mcp::Policy {
                allow_publish: a.allow_publish,
                allow_delete: a.allow_delete,
            })
            .await
        }
    }
}
