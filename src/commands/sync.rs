//! `speck sync` command.

use std::path::{Path, PathBuf};

use crate::context::ServiceContext;
use crate::store::SpecStore;
use crate::sync::beads;

/// Execute the `sync` command with a default live context.
///
/// # Errors
///
/// Returns an error string if sync target is invalid or sync fails.
pub fn run(target: &str, dry_run: bool) -> Result<(), String> {
    let ctx = ServiceContext::live();
    run_with_context(&ctx, target, dry_run, None)
}

/// Execute the `sync` command with a provided service context.
///
/// # Errors
///
/// Returns an error string if sync target is invalid or sync fails.
pub fn run_with_context(
    ctx: &ServiceContext,
    target: &str,
    dry_run: bool,
    override_root: Option<&Path>,
) -> Result<(), String> {
    if target != "beads" {
        return Err(format!("Unknown sync target: {target}. Supported targets: beads"));
    }

    let root = match override_root {
        Some(r) => r.to_path_buf(),
        None => store_root(),
    };
    let store = SpecStore::new(ctx, &root);

    let spec_ids = store.list_task_specs()?;
    if spec_ids.is_empty() {
        println!("No specs found in store.");
        return Ok(());
    }

    let mut specs = Vec::new();
    for id in &spec_ids {
        specs.push(store.load_task_spec(id)?);
    }

    let existing_issues =
        ctx.issues.list_issues(None).map_err(|e| format!("Failed to list existing issues: {e}"))?;

    let actions = beads::plan_sync(&specs, &existing_issues);

    if dry_run {
        println!("Dry run — would perform:");
        println!("{}", beads::format_actions(&actions));
        return Ok(());
    }

    beads::execute_sync(ctx, &specs, &actions)?;
    println!("Sync complete:");
    println!("{}", beads::format_actions(&actions));
    Ok(())
}

fn store_root() -> PathBuf {
    std::env::var("SPECK_STORE").map_or_else(|_| PathBuf::from(".speck"), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::config::CassetteConfig;

    fn test_context() -> ServiceContext {
        let mut ctx = ServiceContext::replaying_from(&CassetteConfig::panic_on_unspecified())
            .expect("panic config should always succeed");
        ctx.fs = Box::new(crate::adapters::live::filesystem::LiveFileSystem);
        ctx
    }

    #[test]
    fn sync_rejects_unknown_target() {
        let ctx = test_context();
        let result = run_with_context(&ctx, "unknown", false, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown sync target"));
    }

    #[test]
    fn sync_dry_run_empty_store() {
        let ctx = test_context();
        let dir = PathBuf::from("/tmp/speck_test_sync_empty_nonexistent");
        let result = run_with_context(&ctx, "beads", true, Some(&dir));
        assert!(result.is_ok());
    }
}
