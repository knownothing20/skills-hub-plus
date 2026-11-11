pub mod auto_update_commands;
pub mod common;
pub mod health_commands;
pub mod install_commands;
pub mod library_commands;
pub mod lock_commands;
pub mod onboarding_commands;
pub mod peer_commands;
pub mod setting_commands;
pub mod shared_commands;
pub mod skill_commands;
pub mod stale_commands;
pub mod sync_commands;
pub mod tool_commands;

#[allow(unused_imports)]
pub use auto_update_commands::*;
#[allow(unused_imports)]
pub use common::*;
#[allow(unused_imports)]
pub use health_commands::*;
#[allow(unused_imports)]
pub use install_commands::*;
#[allow(unused_imports)]
pub use library_commands::*;
#[allow(unused_imports)]
pub use lock_commands::*;
#[allow(unused_imports)]
pub use onboarding_commands::*;
#[allow(unused_imports)]
pub use peer_commands::*;
#[allow(unused_imports)]
pub use setting_commands::*;
#[allow(unused_imports)]
pub use shared_commands::*;
#[allow(unused_imports)]
pub use skill_commands::*;
#[allow(unused_imports)]
pub use stale_commands::*;
#[allow(unused_imports)]
pub use sync_commands::*;
#[allow(unused_imports)]
pub use tool_commands::*;

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
