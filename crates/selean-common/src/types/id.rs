//! Strongly-typed ID newtypes.
//!
//! Each ID type is a newtype wrapper around `Uuid`, providing compile-time
//! type safety so that a `NodeId` cannot be accidentally used where a
//! `TokenId` is expected.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generates a strongly-typed newtype wrapper around `Uuid`.
///
/// The generated type implements `Display`, `Debug`, `Clone`, `Copy`, `Hash`,
/// `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Serialize`, and `Deserialize`.
/// It also provides `new()` for generating a random v4 UUID and `from_uuid()`
/// for wrapping an existing UUID.
macro_rules! define_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Creates a new random ID using UUID v4.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wraps an existing UUID as this ID type.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Returns the inner UUID value.
            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Creates an ID from a nil (all-zeros) UUID. Useful for testing
            /// and as a sentinel value.
            #[must_use]
            pub const fn nil() -> Self {
                Self(Uuid::nil())
            }

            /// Returns `true` if this is the nil (all-zeros) ID.
            #[must_use]
            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id!(
    /// Unique identifier for a design node (frame, text, vector, group, etc.).
    NodeId
);

define_id!(
    /// Unique identifier for a design token (color, spacing, typography, etc.).
    TokenId
);

define_id!(
    /// Unique identifier for a project.
    ProjectId
);

define_id!(
    /// Unique identifier for a page within a project.
    PageId
);

define_id!(
    /// Unique identifier for a component definition.
    ComponentId
);

define_id!(
    /// Unique identifier for a user.
    UserId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_are_unique() {
        let a = NodeId::new();
        let b = NodeId::new();
        assert_ne!(a, b, "Two new IDs should be unique");
    }

    #[test]
    fn nil_id_is_consistent() {
        let a = NodeId::nil();
        let b = NodeId::nil();
        assert_eq!(a, b, "Nil IDs should be equal");
        assert!(a.is_nil());
    }

    #[test]
    fn non_nil_id_is_not_nil() {
        let id = NodeId::new();
        assert!(!id.is_nil());
    }

    #[test]
    fn from_uuid_roundtrip() {
        let uuid = Uuid::new_v4();
        let node_id = NodeId::from_uuid(uuid);
        assert_eq!(*node_id.as_uuid(), uuid);

        let back: Uuid = node_id.into();
        assert_eq!(back, uuid);
    }

    #[test]
    fn different_id_types_are_distinct() {
        // This test validates that the type system prevents mixing ID types.
        // If this compiles, the types are correctly defined as distinct newtypes.
        let uuid = Uuid::new_v4();
        let node_id = NodeId::from_uuid(uuid);
        let token_id = TokenId::from_uuid(uuid);

        // Same underlying UUID, but different types — cannot be compared directly.
        // We verify they wrap the same value via as_uuid().
        assert_eq!(node_id.as_uuid(), token_id.as_uuid());
    }

    #[test]
    fn display_format() {
        let uuid = Uuid::nil();
        let id = NodeId::from_uuid(uuid);
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn debug_format_includes_type_name() {
        let uuid = Uuid::nil();
        let id = NodeId::from_uuid(uuid);
        let debug = format!("{id:?}");
        assert!(
            debug.starts_with("NodeId("),
            "Debug format should include type name, got: {debug}"
        );
    }

    #[test]
    fn serde_roundtrip() {
        let id = NodeId::new();
        let json = serde_json::to_string(&id)
            .unwrap_or_else(|e| panic!("Failed to serialize NodeId: {e}"));
        let deserialized: NodeId = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("Failed to deserialize NodeId: {e}"));
        assert_eq!(id, deserialized);
    }

    #[test]
    fn ordering_is_consistent() {
        let a = NodeId::from_uuid(Uuid::from_u128(1));
        let b = NodeId::from_uuid(Uuid::from_u128(2));
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn hash_is_consistent_with_equality() {
        use std::collections::HashSet;
        let uuid = Uuid::new_v4();
        let a = NodeId::from_uuid(uuid);
        let b = NodeId::from_uuid(uuid);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b), "Equal IDs should have equal hashes");
    }

    #[test]
    fn copy_semantics() {
        let a = NodeId::new();
        let b = a; // Copy, not move
        assert_eq!(a, b, "Copy should produce equal value");
    }
}
