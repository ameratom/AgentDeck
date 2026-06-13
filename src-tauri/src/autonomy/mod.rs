pub mod command_runner;
pub mod composer;
pub mod composer_bridge;
pub mod overnight;
pub mod policy;

pub use command_runner::{run_guarded, AuditEntry, GuardOutcome};
pub use composer::{invoke_composer, ComposerError, ComposerRequest, ComposerResponse};
pub use composer_bridge::{
    apply_unified_patch, bridge_kind_from_env, invoke_with_bridge_kind, ComposerBridgeKind,
};
pub use overnight::{
    load_queue, overnight_branch_name, render_report_markdown, run_overnight, write_report,
    OvernightReport, OvernightTask,
};
pub use policy::{classify_branch_name, classify_shell_command, PolicyVerdict};