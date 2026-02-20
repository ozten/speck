//! `speck sync` command.

use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::store::SpecStore;
use crate::sync::beads;

/// Execute the `sync` command.
///
/// # Errors
///
/// Returns an error string if sync target is invalid or sync fails.
pub fn run(target: &str, dry_run: bool) -> Result<(), String> {
    if target != "beads" {
        return Err(format!("Unknown sync target: {target}. Supported targets: beads"));
    }

    let ctx = ServiceContext::live();
    let store_root = store_root();
    let store = SpecStore::new(&ctx, &store_root);

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

    beads::execute_sync(&ctx, &specs, &actions)?;
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

    #[test]
    fn sync_rejects_unknown_target() {
        let result = run("unknown", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown sync target"));
    }

    #[test]
    fn sync_dry_run_empty_store() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_sync_empty_nonexistent");
        let result = run("beads", true);
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_ok());
    }
}
