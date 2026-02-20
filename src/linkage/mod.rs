//! Linkage resolution and drift detection.
//!
//! The linkage layer maps abstract module references in task specs to concrete
//! file paths in the codebase map. When the codebase changes, drift detection
//! identifies which specs are affected and whether re-planning is needed.

pub mod drift;
pub mod resolve;

pub use drift::{detect_drift, format_drift_report, DriftEntry, DriftReport};
pub use resolve::{resolve, LinkageResult, ResolvedLink};
