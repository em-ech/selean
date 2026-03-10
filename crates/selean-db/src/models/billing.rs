//! Billing tier model and quota limits.

use serde::{Deserialize, Serialize};

/// Billing tier for a workspace. Controls quota limits for members,
/// documents, and storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillingTier {
    /// Free tier with basic limits.
    Free,
    /// Pro tier with expanded limits.
    Pro,
    /// Team tier for larger organizations.
    Team,
    /// Enterprise tier with no enforced limits.
    Enterprise,
}

/// 100 MB in bytes.
const FREE_STORAGE: u64 = 100 * 1024 * 1024;
/// 5 GB in bytes.
const PRO_STORAGE: u64 = 5 * 1024 * 1024 * 1024;
/// 50 GB in bytes.
const TEAM_STORAGE: u64 = 50 * 1024 * 1024 * 1024;

impl BillingTier {
    /// Parses a billing tier from its database string representation.
    #[must_use]
    pub fn from_str_tier(s: &str) -> Option<Self> {
        match s {
            "free" => Some(Self::Free),
            "pro" => Some(Self::Pro),
            "team" => Some(Self::Team),
            "enterprise" => Some(Self::Enterprise),
            _ => None,
        }
    }

    /// Returns the database string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Team => "team",
            Self::Enterprise => "enterprise",
        }
    }

    /// Maximum number of members allowed in a workspace on this tier.
    ///
    /// Returns `None` for unlimited (Enterprise).
    #[must_use]
    pub const fn max_members(&self) -> Option<u32> {
        match self {
            Self::Free => Some(3),
            Self::Pro => Some(5),
            Self::Team => Some(25),
            Self::Enterprise => None,
        }
    }

    /// Maximum number of documents allowed in a workspace on this tier.
    ///
    /// Returns `None` for unlimited (Enterprise).
    #[must_use]
    pub const fn max_documents(&self) -> Option<u32> {
        match self {
            Self::Free => Some(10),
            Self::Pro => Some(100),
            Self::Team => Some(1000),
            Self::Enterprise => None,
        }
    }

    /// Maximum storage in bytes allowed for a workspace on this tier.
    ///
    /// Returns `None` for unlimited (Enterprise).
    #[must_use]
    pub const fn max_storage_bytes(&self) -> Option<u64> {
        match self {
            Self::Free => Some(FREE_STORAGE),
            Self::Pro => Some(PRO_STORAGE),
            Self::Team => Some(TEAM_STORAGE),
            Self::Enterprise => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_tiers() {
        for tier in [
            BillingTier::Free,
            BillingTier::Pro,
            BillingTier::Team,
            BillingTier::Enterprise,
        ] {
            let s = tier.as_str();
            let parsed = BillingTier::from_str_tier(s).expect("valid tier string");
            assert_eq!(parsed, tier);
        }
    }

    #[test]
    fn unknown_tier_returns_none() {
        assert!(BillingTier::from_str_tier("premium").is_none());
        assert!(BillingTier::from_str_tier("").is_none());
    }

    #[test]
    fn free_limits() {
        let tier = BillingTier::Free;
        assert_eq!(tier.max_members(), Some(3));
        assert_eq!(tier.max_documents(), Some(10));
        assert_eq!(tier.max_storage_bytes(), Some(100 * 1024 * 1024));
    }

    #[test]
    fn pro_limits() {
        let tier = BillingTier::Pro;
        assert_eq!(tier.max_members(), Some(5));
        assert_eq!(tier.max_documents(), Some(100));
        assert_eq!(tier.max_storage_bytes(), Some(5 * 1024 * 1024 * 1024));
    }

    #[test]
    fn team_limits() {
        let tier = BillingTier::Team;
        assert_eq!(tier.max_members(), Some(25));
        assert_eq!(tier.max_documents(), Some(1000));
        assert_eq!(tier.max_storage_bytes(), Some(50 * 1024 * 1024 * 1024));
    }

    #[test]
    fn enterprise_unlimited() {
        let tier = BillingTier::Enterprise;
        assert!(tier.max_members().is_none());
        assert!(tier.max_documents().is_none());
        assert!(tier.max_storage_bytes().is_none());
    }

    #[test]
    fn as_str_values() {
        assert_eq!(BillingTier::Free.as_str(), "free");
        assert_eq!(BillingTier::Pro.as_str(), "pro");
        assert_eq!(BillingTier::Team.as_str(), "team");
        assert_eq!(BillingTier::Enterprise.as_str(), "enterprise");
    }
}
