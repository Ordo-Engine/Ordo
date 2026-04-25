//! Platform data models.

#[path = "models/auth.rs"]
mod auth;
#[path = "models/catalog.rs"]
mod catalog;
#[path = "models/deployments.rs"]
mod deployments;
#[path = "models/environments.rs"]
mod environments;
#[path = "models/governance.rs"]
mod governance;
#[path = "models/notifications.rs"]
mod notifications;
#[path = "models/rbac.rs"]
mod rbac;
#[path = "models/release.rs"]
mod release;
#[path = "models/servers.rs"]
mod servers;
#[path = "models/sub_rules.rs"]
mod sub_rules;

pub use auth::*;
pub use catalog::*;
pub use deployments::*;
pub use environments::*;
pub use governance::*;
pub use notifications::*;
pub use rbac::*;
pub use release::*;
pub use servers::*;
pub use sub_rules::*;
