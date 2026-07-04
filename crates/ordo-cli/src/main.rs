use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

mod api;
mod config;
mod deployments;
mod diff;
mod eval;
mod exec;
mod fmt;
mod guard;
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
    /// Agent guardrails — a deterministic policy gate for Claude Code tool calls
    Guard(guard::GuardArgs),

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

    /// Print shell completion script for the given shell
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    if let Err(e) = dispatch(cli.command, json) {
        // Under --json, a hard failure is still machine-readable on stdout so an
        // agent gets structured output on every path, not just the success one.
        if json {
            let _ = output::emit_json(&serde_json::json!({ "error": format!("{e:#}") }));
        } else {
            eprintln!("Error: {e:#}");
        }
        std::process::exit(1);
    }
}

fn dispatch(command: Commands, json: bool) -> Result<()> {
    match command {
        // Local commands run with NO ambient async runtime. The embedded engine's
        // capability invoker uses `reqwest::blocking`, which manages its own runtime;
        // running these under `#[tokio::main]` would panic when that runtime is
        // dropped inside an async context. So we keep the local path fully sync.
        Commands::Init(a) => init::run(a, json),
        Commands::New(a) => new::run(a, json),
        Commands::Eval(a) => eval::run(a, json),
        Commands::Exec(a) => exec::run(a, json),
        Commands::Validate(a) => validate::run(a, json),
        Commands::Trace(a) => trace::run(a, json),
        Commands::Test(a) => test_runner::run(a, json),
        Commands::Fmt(a) => fmt::run(a, json),
        Commands::Lint(a) => lint::run(a, json),
        Commands::Guard(a) => guard::run(a, json),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "ordo", &mut std::io::stdout());
            Ok(())
        }
        // Platform + MCP commands need async; build a runtime only for them.
        Commands::Login(a) => block_on(login::run(a, json)),
        Commands::Whoami => block_on(login::whoami(json)),
        Commands::Link(a) => block_on(link::run(a, json)),
        Commands::Pull(a) => block_on(pull::run(a, json)),
        Commands::Push(a) => block_on(push::run(a, json)),
        Commands::Publish(a) => block_on(publish::run(a, json)),
        Commands::Deployments(a) => block_on(deployments::run(a, json)),
        Commands::Diff(a) => block_on(diff::run(a, json)),
        Commands::Mcp(a) => block_on(mcp::run(mcp::Policy {
            allow_publish: a.allow_publish,
            allow_delete: a.allow_delete,
        })),
    }
}

/// Run an async command on a dedicated multi-thread runtime.
fn block_on<F: std::future::Future<Output = Result<()>>>(fut: F) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?
        .block_on(fut)
}
