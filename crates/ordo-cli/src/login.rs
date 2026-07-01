//! `ordo login` / `ordo whoami` — authenticate against the platform.

use anyhow::{Context, Result};
use clap::Args;
use ordo_api_client::Client;

use crate::config::{self, Config};

#[derive(Args)]
pub struct LoginArgs {
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    password: Option<String>,
    /// API base URL (persisted; default https://api.ordoengine.com/api/v1)
    #[arg(long)]
    api_url: Option<String>,
}

pub async fn run(args: LoginArgs, json: bool) -> Result<()> {
    let mut cfg = Config::load()?;
    if let Some(u) = &args.api_url {
        cfg.api_url = Some(u.trim_end_matches('/').to_string());
    }
    let base = config::resolve_api_url(&cfg, None);

    let email = match args.email {
        Some(e) => e,
        None => prompt("Email: ")?,
    };
    let password = match args.password {
        Some(p) => p,
        None => rpassword::prompt_password("Password: ").context("failed to read password")?,
    };

    let client = Client::new(base.clone(), None);
    let auth = client
        .login(&email, &password)
        .await
        .map_err(|e| anyhow::anyhow!("login failed: {e}"))?;

    cfg.token = Some(auth.token);
    cfg.user_email = Some(auth.user.email.clone());
    cfg.save()?;

    if json {
        crate::output::emit_json(&serde_json::json!({
            "logged_in": true, "email": auth.user.email, "api_url": base,
        }))?;
    } else {
        println!(
            "Logged in as {} ({})",
            auth.user.display_name, auth.user.email
        );
    }
    Ok(())
}

pub async fn whoami(json: bool) -> Result<()> {
    let client = crate::api::authed_client(None)?;
    let user = client.me().await.map_err(|e| anyhow::anyhow!("{e}"))?;
    if json {
        crate::output::emit_json(&serde_json::json!({
            "id": user.id, "email": user.email, "display_name": user.display_name,
        }))?;
    } else {
        println!("{} ({})", user.display_name, user.email);
    }
    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    use std::io::Write;
    print!("{label}");
    std::io::stdout().flush()?;
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}
