//! `speck validate` command.

use std::path::PathBuf;

use crate::adapters::live::filesystem::LiveFileSystem;
use crate::cassette::config::CassetteConfig;
use crate::context::ServiceContext;
use crate::store::SpecStore;
use crate::validate;

/// Execute the `validate` command.
///
/// When `spec_id` is `Some`, validates a single spec.
/// When `all` is `true`, validates every spec in the store.
///
/// # Errors
///
/// Returns an error string if no spec is specified (and `--all` is not set),
/// or if loading/validation fails.
pub fn run(spec_id: Option<&str>, all: bool) -> Result<(), String> {
    let ctx = build_live_context()?;
    let store_root = store_root()?;
    let store = SpecStore::new(&ctx, &store_root);

    let specs = if let Some(id) = spec_id {
        vec![store.load_task_spec(id)?]
    } else if all {
        let ids = store.list_task_specs()?;
        if ids.is_empty() {
            println!("No task specs found in store.");
            return Ok(());
        }
        ids.into_iter().map(|id| store.load_task_spec(&id)).collect::<Result<Vec<_>, _>>()?
    } else {
        return Err("Provide a SPEC_ID or use --all to validate all specs.".to_string());
    };

    let mut any_failed = false;
    for spec in &specs {
        let result = validate::validate_spec(&ctx, spec);
        print!("{}", validate::format_result(&result));
        if !result.passed() {
            any_failed = true;
        }
    }

    if any_failed {
        Err("One or more specs failed validation.".to_string())
    } else {
        Ok(())
    }
}

/// Build a live `ServiceContext` with a real filesystem and panicking stubs
/// for ports that validation doesn't need yet (clock, git, `id_gen`, llm, issues).
fn build_live_context() -> Result<ServiceContext, String> {
    let mut ctx = ServiceContext::replaying_from(&CassetteConfig::panic_on_unspecified())?;
    ctx.fs = Box::new(LiveFileSystem);
    // Shell uses the panicking stub until a live shell adapter exists.
    // TestSuite/CommandOutput checks will fail with a clear panic message.
    Ok(ctx)
}

/// Resolve the spec store root directory.
///
/// Uses `SPECK_STORE` env var if set, otherwise defaults to `.speck/` in the
/// current working directory.
fn store_root() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("SPECK_STORE") {
        return Ok(PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot determine cwd: {e}"))?;
    Ok(cwd.join(".speck"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_command_errors_without_args() {
        let result = run(None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SPEC_ID"));
    }
}
