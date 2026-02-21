//! Live adapter for the `IdGenerator` port.

use uuid::Uuid;

use crate::ports::IdGenerator;

/// Live ID generator that produces random UUIDs.
pub struct LiveIdGenerator;

impl LiveIdGenerator {
    /// Creates a new live ID generator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for LiveIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGenerator for LiveIdGenerator {
    fn generate_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_unique_ids() {
        let gen = LiveIdGenerator::new();
        let id1 = gen.generate_id();
        let id2 = gen.generate_id();

        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID format: 8-4-4-4-12
    }
}
