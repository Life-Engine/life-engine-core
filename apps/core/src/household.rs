//! Household management for multi-user support.
//!
//! A household represents a group of users sharing a single Core instance.
//! Each user has a role (Admin, Member, or Guest) that determines their
//! access level. Data is isolated per-user by default, with designated
//! "shared collections" accessible by all household members.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::auth::types::HouseholdRole;

/// A household grouping multiple users on a single Core instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Household {
    /// Unique household identifier.
    pub id: String,
    /// Human-readable household name.
    pub name: String,
    /// Members of this household.
    pub members: Vec<HouseholdMember>,
    /// Collections shared between all household members.
    pub shared_collections: Vec<String>,
    /// When the household was created.
    pub created_at: DateTime<Utc>,
    /// When the household was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A member within a household.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseholdMember {
    /// The user's unique identifier (OIDC subject).
    pub user_id: String,
    /// Display name.
    pub display_name: String,
    /// Email address (used for invites).
    pub email: Option<String>,
    /// Role within the household.
    pub role: HouseholdRole,
    /// When the member joined.
    pub joined_at: DateTime<Utc>,
}

/// Invite to join a household.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseholdInvite {
    /// Unique invite identifier.
    pub id: String,
    /// The household being invited to.
    pub household_id: String,
    /// Email address the invite was sent to.
    pub email: String,
    /// Role the invitee will receive.
    pub role: HouseholdRole,
    /// Who created the invite.
    pub invited_by: String,
    /// When the invite was created.
    pub created_at: DateTime<Utc>,
    /// When the invite expires.
    pub expires_at: DateTime<Utc>,
    /// Whether the invite has been accepted.
    pub accepted: bool,
}

/// In-memory household store for managing households.
pub struct HouseholdStore {
    households: Arc<RwLock<HashMap<String, Household>>>,
    invites: Arc<RwLock<HashMap<String, HouseholdInvite>>>,
    /// Maps user_id -> household_id for fast lookup.
    user_household_map: Arc<RwLock<HashMap<String, String>>>,
}

impl HouseholdStore {
    /// Create a new empty household store.
    pub fn new() -> Self {
        Self {
            households: Arc::new(RwLock::new(HashMap::new())),
            invites: Arc::new(RwLock::new(HashMap::new())),
            user_household_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new household with the given user as admin.
    pub async fn create_household(
        &self,
        name: &str,
        admin_user_id: &str,
        admin_display_name: &str,
    ) -> Household {
        let now = Utc::now();
        let household_id = uuid::Uuid::new_v4().to_string();
        let household = Household {
            id: household_id.clone(),
            name: name.to_string(),
            members: vec![HouseholdMember {
                user_id: admin_user_id.to_string(),
                display_name: admin_display_name.to_string(),
                email: None,
                role: HouseholdRole::Admin,
                joined_at: now,
            }],
            shared_collections: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        self.households
            .write()
            .await
            .insert(household_id.clone(), household.clone());
        self.user_household_map
            .write()
            .await
            .insert(admin_user_id.to_string(), household_id);
        household
    }

    /// Get a household by ID.
    pub async fn get_household(&self, id: &str) -> Option<Household> {
        self.households.read().await.get(id).cloned()
    }

    /// Get the household for a given user.
    pub async fn get_user_household(&self, user_id: &str) -> Option<Household> {
        let map = self.user_household_map.read().await;
        let household_id = map.get(user_id)?;
        self.households.read().await.get(household_id).cloned()
    }

    /// Get a user's role in their household.
    pub async fn get_user_role(&self, user_id: &str) -> Option<HouseholdRole> {
        let household = self.get_user_household(user_id).await?;
        household
            .members
            .iter()
            .find(|m| m.user_id == user_id)
            .map(|m| m.role)
    }

    /// Create an invite for a new member.
    pub async fn create_invite(
        &self,
        household_id: &str,
        email: &str,
        role: HouseholdRole,
        invited_by: &str,
    ) -> Option<HouseholdInvite> {
        let households = self.households.read().await;
        if !households.contains_key(household_id) {
            return None;
        }
        drop(households);

        let now = Utc::now();
        let invite = HouseholdInvite {
            id: uuid::Uuid::new_v4().to_string(),
            household_id: household_id.to_string(),
            email: email.to_string(),
            role,
            invited_by: invited_by.to_string(),
            created_at: now,
            expires_at: now + chrono::Duration::days(7),
            accepted: false,
        };
        self.invites
            .write()
            .await
            .insert(invite.id.clone(), invite.clone());
        Some(invite)
    }

    /// Accept an invite and add the user to the household.
    pub async fn accept_invite(
        &self,
        invite_id: &str,
        user_id: &str,
        display_name: &str,
    ) -> Result<HouseholdMember, String> {
        let mut invites = self.invites.write().await;
        let invite = invites
            .get_mut(invite_id)
            .ok_or_else(|| "invite not found".to_string())?;

        if invite.accepted {
            return Err("invite already accepted".to_string());
        }
        if invite.expires_at < Utc::now() {
            return Err("invite expired".to_string());
        }

        invite.accepted = true;
        let household_id = invite.household_id.clone();
        let role = invite.role;
        drop(invites);

        let member = HouseholdMember {
            user_id: user_id.to_string(),
            display_name: display_name.to_string(),
            email: None,
            role,
            joined_at: Utc::now(),
        };

        let mut households = self.households.write().await;
        if let Some(household) = households.get_mut(&household_id) {
            household.members.push(member.clone());
            household.updated_at = Utc::now();
        }
        drop(households);

        self.user_household_map
            .write()
            .await
            .insert(user_id.to_string(), household_id);

        Ok(member)
    }

    /// Add a collection to the household's shared collections.
    pub async fn add_shared_collection(
        &self,
        household_id: &str,
        collection: &str,
    ) -> Result<(), String> {
        let mut households = self.households.write().await;
        let household = households
            .get_mut(household_id)
            .ok_or_else(|| "household not found".to_string())?;

        if !household.shared_collections.contains(&collection.to_string()) {
            household.shared_collections.push(collection.to_string());
            household.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Check if a collection is shared in a household.
    pub async fn is_shared_collection(
        &self,
        household_id: &str,
        collection: &str,
    ) -> bool {
        let households = self.households.read().await;
        households
            .get(household_id)
            .map(|h| h.shared_collections.contains(&collection.to_string()))
            .unwrap_or(false)
    }

    /// Check if a user can access a record based on household rules.
    ///
    /// Delegates to the pure [`check_record_access`] function after
    /// resolving the user's household context from the store.
    pub async fn can_access_record(
        &self,
        requesting_user_id: &str,
        record_user_id: Option<&str>,
        record_household_id: Option<&str>,
        collection: &str,
    ) -> bool {
        let user_household = self.get_user_household(requesting_user_id).await;
        let (user_hid, user_role, shared_cols) = match &user_household {
            Some(h) => {
                let role = h
                    .members
                    .iter()
                    .find(|m| m.user_id == requesting_user_id)
                    .map(|m| m.role);
                (Some(h.id.as_str()), role, h.shared_collections.as_slice())
            }
            None => (None, None, &[] as &[String]),
        };

        check_record_access(
            requesting_user_id,
            user_role,
            user_hid,
            record_user_id,
            record_household_id,
            collection,
            shared_cols,
        )
    }
}

impl Default for HouseholdStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Pure permission check: can `requesting_user_id` access a record?
///
/// This is extracted as a standalone function so it can be reused across
/// the household store, data routes, and plugin isolation layers without
/// needing a reference to the full `HouseholdStore`.
///
/// Returns `true` if access is allowed.
pub fn check_record_access(
    requesting_user_id: &str,
    requesting_user_role: Option<HouseholdRole>,
    requesting_user_household_id: Option<&str>,
    record_user_id: Option<&str>,
    record_household_id: Option<&str>,
    collection: &str,
    shared_collections: &[String],
) -> bool {
    // Legacy data without user scoping is accessible to all.
    let record_owner = match record_user_id {
        Some(uid) => uid,
        None => return true,
    };

    // Owner always has access.
    if record_owner == requesting_user_id {
        return true;
    }

    // Must be in the same household to access non-owned data.
    let record_hid = match record_household_id {
        Some(hid) => hid,
        None => return false,
    };

    let user_hid = match requesting_user_household_id {
        Some(hid) if hid == record_hid => hid,
        _ => return false,
    };

    // Shared collections are accessible to all household members.
    if shared_collections.contains(&collection.to_string()) {
        return true;
    }

    // Admins can access all data within their household.
    matches!(requesting_user_role, Some(HouseholdRole::Admin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn user_a_cannot_read_user_b_private_data() {
        let store = HouseholdStore::new();

        // Create a household with user A as admin.
        let household = store
            .create_household("Test Family", "user-a", "User A")
            .await;

        // Invite and add user B as member.
        let invite = store
            .create_invite(&household.id, "b@test.com", HouseholdRole::Member, "user-a")
            .await
            .unwrap();
        store
            .accept_invite(&invite.id, "user-b", "User B")
            .await
            .unwrap();

        // User B should NOT be able to access user A's private data
        // in a non-shared collection.
        let can_access = store
            .can_access_record("user-b", Some("user-a"), Some(&household.id), "private-notes")
            .await;
        assert!(
            !can_access,
            "member should not access another member's private data"
        );
    }

    #[tokio::test]
    async fn user_can_access_own_data() {
        let store = HouseholdStore::new();
        let household = store
            .create_household("Test Family", "user-a", "User A")
            .await;

        let can_access = store
            .can_access_record("user-a", Some("user-a"), Some(&household.id), "private-notes")
            .await;
        assert!(can_access, "user should access their own data");
    }

    #[tokio::test]
    async fn shared_collections_accessible_by_all_members() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "user-a", "User A")
            .await;

        // Add user B as member.
        let invite = store
            .create_invite(&household.id, "b@test.com", HouseholdRole::Member, "user-a")
            .await
            .unwrap();
        store
            .accept_invite(&invite.id, "user-b", "User B")
            .await
            .unwrap();

        // Mark "family-calendar" as a shared collection.
        store
            .add_shared_collection(&household.id, "family-calendar")
            .await
            .unwrap();

        // User B can access user A's data in the shared collection.
        let can_access = store
            .can_access_record(
                "user-b",
                Some("user-a"),
                Some(&household.id),
                "family-calendar",
            )
            .await;
        assert!(
            can_access,
            "member should access shared collection data"
        );

        // But not in a non-shared collection.
        let cannot_access = store
            .can_access_record(
                "user-b",
                Some("user-a"),
                Some(&household.id),
                "private-notes",
            )
            .await;
        assert!(
            !cannot_access,
            "member should not access non-shared collection data"
        );
    }

    #[tokio::test]
    async fn role_based_access_admin_can_access_all() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "admin-user", "Admin")
            .await;

        // Add a member.
        let invite = store
            .create_invite(
                &household.id,
                "member@test.com",
                HouseholdRole::Member,
                "admin-user",
            )
            .await
            .unwrap();
        store
            .accept_invite(&invite.id, "member-user", "Member")
            .await
            .unwrap();

        // Admin can access member's private data.
        let can_access = store
            .can_access_record(
                "admin-user",
                Some("member-user"),
                Some(&household.id),
                "private-notes",
            )
            .await;
        assert!(can_access, "admin should access all household data");
    }

    #[tokio::test]
    async fn role_based_access_guest_read_only_shared() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "admin-user", "Admin")
            .await;

        // Add a guest.
        let invite = store
            .create_invite(
                &household.id,
                "guest@test.com",
                HouseholdRole::Guest,
                "admin-user",
            )
            .await
            .unwrap();
        store
            .accept_invite(&invite.id, "guest-user", "Guest")
            .await
            .unwrap();

        // Mark "shopping-list" as shared.
        store
            .add_shared_collection(&household.id, "shopping-list")
            .await
            .unwrap();

        // Guest can access shared collection.
        let can_access = store
            .can_access_record(
                "guest-user",
                Some("admin-user"),
                Some(&household.id),
                "shopping-list",
            )
            .await;
        assert!(can_access, "guest should access shared collections");

        // Guest cannot access non-shared data.
        let cannot_access = store
            .can_access_record(
                "guest-user",
                Some("admin-user"),
                Some(&household.id),
                "private-notes",
            )
            .await;
        assert!(
            !cannot_access,
            "guest should not access non-shared collections"
        );
    }

    #[tokio::test]
    async fn invite_flow_creates_member_with_correct_role() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "admin", "Admin User")
            .await;

        // Create invite with Member role.
        let invite = store
            .create_invite(&household.id, "new@test.com", HouseholdRole::Member, "admin")
            .await
            .unwrap();
        assert_eq!(invite.role, HouseholdRole::Member);
        assert!(!invite.accepted);

        // Accept the invite.
        let member = store
            .accept_invite(&invite.id, "new-user", "New User")
            .await
            .unwrap();
        assert_eq!(member.role, HouseholdRole::Member);
        assert_eq!(member.user_id, "new-user");

        // Verify the user is in the household with correct role.
        let role = store.get_user_role("new-user").await;
        assert_eq!(role, Some(HouseholdRole::Member));

        // Verify household now has 2 members.
        let updated = store.get_household(&household.id).await.unwrap();
        assert_eq!(updated.members.len(), 2);
    }

    #[tokio::test]
    async fn invite_for_nonexistent_household_fails() {
        let store = HouseholdStore::new();
        let invite = store
            .create_invite("nonexistent", "user@test.com", HouseholdRole::Member, "admin")
            .await;
        assert!(invite.is_none());
    }

    #[tokio::test]
    async fn expired_invite_cannot_be_accepted() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "admin", "Admin")
            .await;

        let invite = store
            .create_invite(&household.id, "user@test.com", HouseholdRole::Member, "admin")
            .await
            .unwrap();

        // Manually expire the invite.
        {
            let mut invites = store.invites.write().await;
            let inv = invites.get_mut(&invite.id).unwrap();
            inv.expires_at = Utc::now() - chrono::Duration::hours(1);
        }

        let result = store
            .accept_invite(&invite.id, "new-user", "New User")
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "invite expired");
    }

    #[tokio::test]
    async fn duplicate_invite_acceptance_fails() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "admin", "Admin")
            .await;

        let invite = store
            .create_invite(&household.id, "user@test.com", HouseholdRole::Member, "admin")
            .await
            .unwrap();

        // Accept once.
        store
            .accept_invite(&invite.id, "user-1", "User 1")
            .await
            .unwrap();

        // Try to accept again.
        let result = store
            .accept_invite(&invite.id, "user-2", "User 2")
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "invite already accepted");
    }

    #[tokio::test]
    async fn cross_household_access_denied() {
        let store = HouseholdStore::new();

        let household_a = store
            .create_household("Family A", "user-a", "User A")
            .await;
        let _household_b = store
            .create_household("Family B", "user-b", "User B")
            .await;

        // User B should not access user A's data even if they guess the household.
        let can_access = store
            .can_access_record(
                "user-b",
                Some("user-a"),
                Some(&household_a.id),
                "tasks",
            )
            .await;
        assert!(
            !can_access,
            "user from different household should not access data"
        );
    }

    #[tokio::test]
    async fn legacy_data_without_user_id_accessible() {
        let store = HouseholdStore::new();
        let _household = store
            .create_household("Test Family", "user-a", "User A")
            .await;

        // Records without user_id (legacy) should be accessible to all.
        let can_access = store
            .can_access_record("user-a", None, None, "tasks")
            .await;
        assert!(can_access, "legacy data should be accessible");
    }

    #[tokio::test]
    async fn guest_invite_creates_guest_role() {
        let store = HouseholdStore::new();

        let household = store
            .create_household("Test Family", "admin", "Admin")
            .await;

        let invite = store
            .create_invite(&household.id, "guest@test.com", HouseholdRole::Guest, "admin")
            .await
            .unwrap();

        let member = store
            .accept_invite(&invite.id, "guest-user", "Guest User")
            .await
            .unwrap();
        assert_eq!(member.role, HouseholdRole::Guest);

        let role = store.get_user_role("guest-user").await;
        assert_eq!(role, Some(HouseholdRole::Guest));
    }

    // ── Pure function tests (check_record_access) ──────────────

    #[test]
    fn pure_check_legacy_data_accessible() {
        assert!(check_record_access(
            "user-a",
            None,
            None,
            None, // no record owner
            None,
            "tasks",
            &[],
        ));
    }

    #[test]
    fn pure_check_owner_access() {
        assert!(check_record_access(
            "user-a",
            Some(HouseholdRole::Member),
            Some("h1"),
            Some("user-a"),
            Some("h1"),
            "tasks",
            &[],
        ));
    }

    #[test]
    fn pure_check_non_owner_denied() {
        assert!(!check_record_access(
            "user-b",
            Some(HouseholdRole::Member),
            Some("h1"),
            Some("user-a"),
            Some("h1"),
            "private-notes",
            &[],
        ));
    }

    #[test]
    fn pure_check_shared_collection_allowed() {
        assert!(check_record_access(
            "user-b",
            Some(HouseholdRole::Member),
            Some("h1"),
            Some("user-a"),
            Some("h1"),
            "family-calendar",
            &["family-calendar".to_string()],
        ));
    }

    #[test]
    fn pure_check_admin_access_all() {
        assert!(check_record_access(
            "admin",
            Some(HouseholdRole::Admin),
            Some("h1"),
            Some("member"),
            Some("h1"),
            "private-notes",
            &[],
        ));
    }

    #[test]
    fn pure_check_cross_household_denied() {
        assert!(!check_record_access(
            "user-b",
            Some(HouseholdRole::Admin),
            Some("h2"), // different household
            Some("user-a"),
            Some("h1"),
            "tasks",
            &[],
        ));
    }

    #[test]
    fn pure_check_guest_shared_only() {
        // Guest can access shared collections.
        assert!(check_record_access(
            "guest",
            Some(HouseholdRole::Guest),
            Some("h1"),
            Some("admin"),
            Some("h1"),
            "shopping-list",
            &["shopping-list".to_string()],
        ));

        // Guest cannot access non-shared collections.
        assert!(!check_record_access(
            "guest",
            Some(HouseholdRole::Guest),
            Some("h1"),
            Some("admin"),
            Some("h1"),
            "private-notes",
            &[],
        ));
    }
}
