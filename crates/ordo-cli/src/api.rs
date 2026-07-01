//! Auth-context resolution + client construction for the platform commands.

use anyhow::{Context, Result};
use ordo_api_client::Client;

use crate::config::{self, Config};
use crate::project::Project;

/// Build an authenticated client (token required — errors if not logged in).
pub fn authed_client(project_api_url: Option<&str>) -> Result<Client> {
    let cfg = Config::load()?;
    let token = config::resolve_token(&cfg)
        .context("not logged in — run `ordo login` (or set ORDO_TOKEN)")?;
    let base = config::resolve_api_url(&cfg, project_api_url);
    Ok(Client::new(base, Some(token)))
}

/// A client plus the linked org/project ids from `ordo.yaml`.
pub struct Linked {
    pub client: Client,
    pub org_id: String,
    pub project_id: String,
}

/// Resolve the client + org/project link for a project. Errors if `ordo link`
/// hasn't been run.
pub fn linked(project: &Project) -> Result<Linked> {
    let org_id = project
        .config
        .org_id
        .clone()
        .context("project is not linked to the platform — run `ordo link`")?;
    let project_id = project
        .config
        .project_id
        .clone()
        .context("project is not linked to the platform — run `ordo link`")?;
    let client = authed_client(project.config.api_url.as_deref())?;
    Ok(Linked {
        client,
        org_id,
        project_id,
    })
}
