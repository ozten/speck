//! ID generator port for producing unique identifiers.

/// Generates unique identifiers.
///
/// Abstracting ID generation allows deterministic replay by substituting
/// a predictable sequence during tests and cassette playback.
pub trait IdGenerator: Send + Sync {
    /// Generates a new unique identifier string.
    fn generate_id(&self) -> String;
}
