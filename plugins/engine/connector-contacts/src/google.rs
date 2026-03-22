//! Google Contacts connector with OAuth2 token management and incremental sync.
//!
//! Provides configuration, client types, and API methods for syncing contacts
//! via the Google People API. Supports OAuth2 token refresh, paginated listing
//! with incremental sync via syncToken, and CRUD operations on contacts.

use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use life_engine_types::{Contact, ContactName, EmailAddress, PhoneNumber, PostalAddress};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The base URL for the Google People API v1.
const PEOPLE_API_BASE: &str = "https://people.googleapis.com/v1";

/// The Google OAuth2 token endpoint.
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// The personFields parameter value for People API requests.
const PERSON_FIELDS: &str = "names,emailAddresses,phoneNumbers,addresses,organizations";

/// Response from the Google People API `people.connections.list` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleListResponse {
    /// The list of person resources (contacts).
    #[serde(default)]
    pub connections: Vec<GooglePerson>,
    /// Token for fetching the next page, if more results exist.
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
    /// Sync token for incremental sync on subsequent requests.
    #[serde(rename = "nextSyncToken", default)]
    pub next_sync_token: Option<String>,
    /// Total number of contacts (approximate, from the API).
    #[serde(rename = "totalPeople", default)]
    pub total_people: Option<u32>,
}

/// Configuration for Google Contacts API access.
///
/// Secrets (client_secret, refresh_token) are stored in the credential
/// store, referenced by their key names here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleContactsConfig {
    /// OAuth2 client ID.
    pub client_id: String,
    /// The key used to look up the client secret in the credential store.
    #[serde(default = "default_google_contacts_client_secret_key")]
    pub client_secret_key: String,
    /// The key used to look up the refresh token in the credential store.
    #[serde(default = "default_google_contacts_refresh_token_key")]
    pub refresh_token_key: String,
}

/// Default credential key for Google Contacts client secret.
fn default_google_contacts_client_secret_key() -> String {
    "google_contacts_client_secret".to_string()
}

/// Default credential key for Google Contacts refresh token.
fn default_google_contacts_refresh_token_key() -> String {
    "google_contacts_refresh_token".to_string()
}

/// Sync state for Google Contacts incremental sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoogleSyncState {
    /// The sync token from the last People API list response.
    pub sync_token: Option<String>,
    /// Timestamp of the last successful sync.
    pub last_sync: Option<DateTime<Utc>>,
}

/// Cached OAuth2 access token with expiration tracking.
#[derive(Debug, Clone)]
pub struct OAuthTokenCache {
    /// The current access token.
    pub access_token: String,
    /// When the access token expires.
    pub expires_at: DateTime<Utc>,
}

impl OAuthTokenCache {
    /// Returns `true` if the cached token is expired or will expire
    /// within the next 60 seconds.
    pub fn is_expired(&self) -> bool {
        Utc::now() + Duration::seconds(60) >= self.expires_at
    }
}

/// Response from the Google OAuth2 token endpoint.
#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[allow(dead_code)]
    token_type: String,
}

/// Google Contacts client for syncing contacts via the People API.
///
/// Manages OAuth2 configuration, token caching, an HTTP client, and sync
/// state. Provides methods for listing, getting, creating, updating, and
/// deleting contacts through the Google People API.
pub struct GoogleContactsClient {
    /// OAuth2 configuration.
    config: GoogleContactsConfig,
    /// Sync state for incremental sync.
    sync_state: GoogleSyncState,
    /// Cached OAuth2 access token.
    token_cache: Option<OAuthTokenCache>,
    /// Reusable HTTP client.
    pub http_client: reqwest::Client,
    /// The People API base URL (overridable for testing).
    api_base_url: String,
    /// The OAuth2 token endpoint URL (overridable for testing).
    token_endpoint_url: String,
}

impl GoogleContactsClient {
    /// Create a new Google Contacts client with the given configuration.
    pub fn new(config: GoogleContactsConfig) -> Self {
        Self {
            config,
            sync_state: GoogleSyncState::default(),
            token_cache: None,
            http_client: reqwest::Client::new(),
            api_base_url: PEOPLE_API_BASE.to_string(),
            token_endpoint_url: TOKEN_ENDPOINT.to_string(),
        }
    }

    /// Create a new client with custom endpoint URLs (for testing).
    #[cfg(test)]
    fn with_endpoints(
        config: GoogleContactsConfig,
        api_base_url: String,
        token_endpoint_url: String,
    ) -> Self {
        Self {
            config,
            sync_state: GoogleSyncState::default(),
            token_cache: None,
            http_client: reqwest::Client::new(),
            api_base_url,
            token_endpoint_url,
        }
    }

    /// Returns the Google Contacts configuration.
    pub fn config(&self) -> &GoogleContactsConfig {
        &self.config
    }

    /// Returns the current sync state.
    pub fn sync_state(&self) -> &GoogleSyncState {
        &self.sync_state
    }

    /// Returns the cached token, if present.
    pub fn token_cache(&self) -> Option<&OAuthTokenCache> {
        self.token_cache.as_ref()
    }

    /// Update the sync token after a successful sync.
    pub fn update_sync_token(&mut self, token: String) {
        self.sync_state.sync_token = Some(token);
        self.sync_state.last_sync = Some(Utc::now());
    }

    /// Reset sync state for a full re-sync.
    pub fn reset_sync_state(&mut self) {
        self.sync_state = GoogleSyncState::default();
    }

    /// Ensure a valid OAuth2 access token is available.
    ///
    /// Returns the cached access token if it has not expired (with a 60-second
    /// safety margin). Otherwise, refreshes the token using the provided
    /// credentials by POSTing to the Google OAuth2 token endpoint.
    ///
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    pub async fn ensure_valid_token(
        &mut self,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<String> {
        if let Some(ref cache) = self.token_cache
            && !cache.is_expired()
        {
            return Ok(cache.access_token.clone());
        }

        tracing::debug!("refreshing Google Contacts OAuth2 access token");

        let response = self
            .http_client
            .post(&self.token_endpoint_url)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", client_secret),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .context("failed to send token refresh request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("token refresh failed with {status}: {body}");
        }

        let token_resp: TokenResponse = response
            .json()
            .await
            .context("failed to parse token refresh response")?;

        let cache = OAuthTokenCache {
            access_token: token_resp.access_token.clone(),
            expires_at: Utc::now() + Duration::seconds(token_resp.expires_in),
        };
        self.token_cache = Some(cache);

        Ok(token_resp.access_token)
    }

    /// List contacts from the Google People API with pagination and incremental sync.
    ///
    /// Uses the stored sync token (if available) for delta syncs. Handles
    /// pagination via `nextPageToken`, collecting all pages into a single
    /// result. On HTTP 410 Gone (expired sync token), clears the sync state
    /// and retries as a full sync.
    ///
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    ///
    /// Returns all contacts across all pages as `GooglePerson` values.
    pub async fn list_connections(
        &mut self,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<Vec<GooglePerson>> {
        let sync_token = self.sync_state.sync_token.clone();

        match self
            .list_connections_inner(sync_token.as_deref(), client_secret, refresh_token)
            .await
        {
            Ok(persons) => Ok(persons),
            Err(e) => {
                // Check for 410 Gone indicating expired sync token
                let err_msg = e.to_string();
                if err_msg.contains("410") || err_msg.contains("Gone") {
                    tracing::warn!(
                        "sync token expired (410 Gone), performing full re-sync"
                    );
                    self.reset_sync_state();
                    self.list_connections_inner(None, client_secret, refresh_token)
                        .await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Inner implementation for listing connections with optional sync token.
    async fn list_connections_inner(
        &mut self,
        sync_token: Option<&str>,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<Vec<GooglePerson>> {
        let mut all_persons = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let access_token = self
                .ensure_valid_token(client_secret, refresh_token)
                .await?;

            let mut url = reqwest::Url::parse(&format!(
                "{}/people/me/connections",
                self.api_base_url
            ))?;

            {
                let mut params = url.query_pairs_mut();
                params.append_pair("personFields", PERSON_FIELDS);
                params.append_pair("pageSize", "100");

                if let Some(token) = sync_token {
                    params.append_pair("syncToken", token);
                    // Request deleted contacts during incremental sync
                    params.append_pair("requestSyncToken", "true");
                } else {
                    params.append_pair("requestSyncToken", "true");
                }

                if let Some(ref token) = page_token {
                    params.append_pair("pageToken", token);
                }
            }

            let response = self
                .http_client
                .get(url)
                .bearer_auth(&access_token)
                .send()
                .await
                .context("failed to send People API list request")?;

            if response.status().as_u16() == 410 {
                anyhow::bail!("410 Gone: sync token expired");
            }

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!(
                    "Google People API returned {status}: {body}"
                );
            }

            let list_response: GoogleListResponse = response
                .json()
                .await
                .context("failed to parse People API list response")?;

            all_persons.extend(list_response.connections);

            if let Some(next_page) = list_response.next_page_token {
                page_token = Some(next_page);
            } else {
                // Final page: store the new sync token
                if let Some(token) = list_response.next_sync_token {
                    self.update_sync_token(token);
                }
                break;
            }
        }

        Ok(all_persons)
    }

    /// Get a single contact by resource name.
    ///
    /// The `resource_name` should be in the format `people/c1234567890`.
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    pub async fn get_contact(
        &mut self,
        resource_name: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<GooglePerson> {
        let access_token = self
            .ensure_valid_token(client_secret, refresh_token)
            .await?;

        let url = format!(
            "{}/{resource_name}?personFields={PERSON_FIELDS}",
            self.api_base_url
        );

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&access_token)
            .send()
            .await
            .context("failed to send People API get request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Google People API get returned {status}: {body}"
            );
        }

        let person: GooglePerson = response
            .json()
            .await
            .context("failed to parse People API get response")?;

        Ok(person)
    }

    /// Create a new contact via the People API.
    ///
    /// Sends a POST to `people.createContact` with the given person data.
    /// Returns the created person resource with its server-assigned resource name.
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    pub async fn create_contact(
        &mut self,
        person: &GooglePerson,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<GooglePerson> {
        let access_token = self
            .ensure_valid_token(client_secret, refresh_token)
            .await?;

        let url = format!(
            "{}/people:createContact?personFields={PERSON_FIELDS}",
            self.api_base_url
        );

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&access_token)
            .json(person)
            .send()
            .await
            .context("failed to send People API create request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Google People API create returned {status}: {body}"
            );
        }

        let created: GooglePerson = response
            .json()
            .await
            .context("failed to parse People API create response")?;

        Ok(created)
    }

    /// Update an existing contact via the People API.
    ///
    /// Sends a PATCH to `people.updateContact` for the person's resource name.
    /// The person must have a valid `resource_name` and `etag` set.
    /// Returns the updated person resource.
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    pub async fn update_contact(
        &mut self,
        person: &GooglePerson,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<GooglePerson> {
        let access_token = self
            .ensure_valid_token(client_secret, refresh_token)
            .await?;

        let url = format!(
            "{}/{}:updateContact?updatePersonFields={PERSON_FIELDS}",
            self.api_base_url, person.resource_name
        );

        let response = self
            .http_client
            .patch(&url)
            .bearer_auth(&access_token)
            .json(person)
            .send()
            .await
            .context("failed to send People API update request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Google People API update returned {status}: {body}"
            );
        }

        let updated: GooglePerson = response
            .json()
            .await
            .context("failed to parse People API update response")?;

        Ok(updated)
    }

    /// Delete a contact by resource name.
    ///
    /// Sends a DELETE to `people.deleteContact` for the given resource name.
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    pub async fn delete_contact(
        &mut self,
        resource_name: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<()> {
        let access_token = self
            .ensure_valid_token(client_secret, refresh_token)
            .await?;

        let url = format!(
            "{}/{}:deleteContact",
            self.api_base_url, resource_name
        );

        let response = self
            .http_client
            .delete(&url)
            .bearer_auth(&access_token)
            .send()
            .await
            .context("failed to send People API delete request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Google People API delete returned {status}: {body}"
            );
        }

        Ok(())
    }

    /// Perform a contact sync using the stored sync token.
    ///
    /// On the first call (no stored sync token), performs a full sync.
    /// On subsequent calls, uses the stored sync token for incremental sync.
    /// Maps all returned Google contacts to CDM `Contact` types and updates
    /// the stored sync token.
    ///
    /// The `client_secret` and `refresh_token` parameters should be retrieved
    /// from the credential store using the config's key fields.
    ///
    /// Returns the list of synced contacts.
    pub async fn sync_contacts(
        &mut self,
        client_secret: &str,
        refresh_token: &str,
    ) -> anyhow::Result<Vec<Contact>> {
        let persons = self
            .list_connections(client_secret, refresh_token)
            .await?;

        let contacts: Vec<Contact> = persons
            .iter()
            .map(map_google_person)
            .collect();

        Ok(contacts)
    }

    /// Map a list response's connections to CDM contacts (no HTTP call).
    ///
    /// Useful for processing pre-fetched responses or testing.
    pub fn process_list_response(
        &mut self,
        response: GoogleListResponse,
    ) -> Vec<Contact> {
        let contacts: Vec<Contact> = response
            .connections
            .iter()
            .map(map_google_person)
            .collect();

        if let Some(token) = response.next_sync_token {
            self.update_sync_token(token);
        }

        contacts
    }
}

/// A person resource from the Google People API (simplified).
///
/// This mirrors the relevant subset of the People API response
/// for mapping into our CDM `Contact` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePerson {
    /// The resource name (e.g. `people/c1234567890`).
    #[serde(default)]
    pub resource_name: String,
    /// Display name from the names array.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Given name.
    #[serde(default)]
    pub given_name: Option<String>,
    /// Family name.
    #[serde(default)]
    pub family_name: Option<String>,
    /// Email addresses.
    #[serde(default)]
    pub email_addresses: Vec<GoogleEmailAddress>,
    /// Phone numbers.
    #[serde(default)]
    pub phone_numbers: Vec<GooglePhoneNumber>,
    /// Postal addresses.
    #[serde(default)]
    pub addresses: Vec<GoogleAddress>,
    /// Organisation name.
    #[serde(default)]
    pub organisation: Option<String>,
    /// ETag for conflict detection on updates.
    #[serde(default)]
    pub etag: Option<String>,
}

/// An email address from the People API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleEmailAddress {
    /// The email address value.
    pub value: String,
    /// The type (e.g. "work", "home").
    #[serde(rename = "type", default)]
    pub email_type: Option<String>,
}

/// A phone number from the People API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePhoneNumber {
    /// The phone number value.
    pub value: String,
    /// The type (e.g. "mobile", "work").
    #[serde(rename = "type", default)]
    pub phone_type: Option<String>,
}

/// A postal address from the People API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAddress {
    /// Street address.
    #[serde(default)]
    pub street_address: Option<String>,
    /// City.
    #[serde(default)]
    pub city: Option<String>,
    /// Region/state.
    #[serde(default)]
    pub region: Option<String>,
    /// Postal code.
    #[serde(default)]
    pub postal_code: Option<String>,
    /// Country.
    #[serde(default)]
    pub country: Option<String>,
    /// Address type.
    #[serde(rename = "type", default)]
    pub address_type: Option<String>,
}

/// Map a Google People API person to the Life Engine CDM `Contact`.
pub fn map_google_person(person: &GooglePerson) -> Contact {
    let now = Utc::now();

    let given = person.given_name.clone().unwrap_or_default();
    let family = person.family_name.clone().unwrap_or_default();
    let display = person
        .display_name
        .clone()
        .unwrap_or_else(|| format!("{given} {family}").trim().to_string());

    let emails: Vec<EmailAddress> = person
        .email_addresses
        .iter()
        .map(|e| EmailAddress {
            address: e.value.clone(),
            email_type: e.email_type.clone(),
            primary: None,
        })
        .collect();

    let phones: Vec<PhoneNumber> = person
        .phone_numbers
        .iter()
        .map(|p| PhoneNumber {
            number: p.value.clone(),
            phone_type: p.phone_type.clone(),
        })
        .collect();

    let addresses: Vec<PostalAddress> = person
        .addresses
        .iter()
        .map(|a| PostalAddress {
            street: a.street_address.clone(),
            city: a.city.clone(),
            state: a.region.clone(),
            postcode: a.postal_code.clone(),
            country: a.country.clone(),
        })
        .collect();

    Contact {
        id: Uuid::new_v4(),
        name: ContactName {
            given,
            family,
            display,
        },
        emails,
        phones,
        addresses,
        organisation: person.organisation.clone(),
        source: "google".into(),
        source_id: person.resource_name.clone(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // Factory functions for reusable test fixtures
    // -------------------------------------------------------------------

    /// Returns a default `GoogleContactsConfig` for testing.
    fn test_config() -> GoogleContactsConfig {
        GoogleContactsConfig {
            client_id: "test-client-id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        }
    }

    /// Returns a fully-populated `GooglePerson` for testing.
    fn test_person() -> GooglePerson {
        GooglePerson {
            resource_name: "people/c12345".into(),
            display_name: Some("Jane Doe".into()),
            given_name: Some("Jane".into()),
            family_name: Some("Doe".into()),
            email_addresses: vec![GoogleEmailAddress {
                value: "jane@example.com".into(),
                email_type: Some("work".into()),
            }],
            phone_numbers: vec![GooglePhoneNumber {
                value: "+1-555-0100".into(),
                phone_type: Some("mobile".into()),
            }],
            addresses: vec![GoogleAddress {
                street_address: Some("123 Main St".into()),
                city: Some("Springfield".into()),
                region: Some("IL".into()),
                postal_code: Some("62701".into()),
                country: Some("US".into()),
                address_type: Some("home".into()),
            }],
            organisation: Some("Acme Corp".into()),
            etag: None,
        }
    }

    #[test]
    fn google_config_serialization() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: GoogleContactsConfig =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.client_id, "test-client-id");
        assert_eq!(restored.client_secret_key, "google_contacts_client_secret");
        assert_eq!(restored.refresh_token_key, "google_contacts_refresh_token");
    }

    #[test]
    fn google_client_construction() {
        let client = GoogleContactsClient::new(test_config());
        assert_eq!(client.config().client_id, "test-client-id");
        assert!(client.sync_state().sync_token.is_none());
        assert!(client.sync_state().last_sync.is_none());
    }

    #[test]
    fn google_sync_state_update() {
        let mut client = GoogleContactsClient::new(test_config());

        client.update_sync_token("sync-abc".into());
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("sync-abc")
        );
        assert!(client.sync_state().last_sync.is_some());
    }

    #[test]
    fn google_sync_state_reset() {
        let mut client = GoogleContactsClient::new(test_config());

        client.update_sync_token("sync-abc".into());
        client.reset_sync_state();
        assert!(client.sync_state().sync_token.is_none());
        assert!(client.sync_state().last_sync.is_none());
    }

    #[test]
    fn map_google_person_full() {
        let person = test_person();
        let contact = map_google_person(&person);
        assert_eq!(contact.name.display, "Jane Doe");
        assert_eq!(contact.name.given, "Jane");
        assert_eq!(contact.name.family, "Doe");
        assert_eq!(contact.source, "google");
        assert_eq!(contact.source_id, "people/c12345");
        assert_eq!(contact.emails.len(), 1);
        assert_eq!(contact.emails[0].address, "jane@example.com");
        assert_eq!(contact.phones.len(), 1);
        assert_eq!(contact.phones[0].number, "+1-555-0100");
        assert_eq!(contact.addresses.len(), 1);
        assert_eq!(
            contact.addresses[0].street.as_deref(),
            Some("123 Main St")
        );
        assert_eq!(contact.organisation.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn map_google_person_minimal() {
        let person = GooglePerson {
            resource_name: "people/c99999".into(),
            display_name: None,
            given_name: Some("Solo".into()),
            family_name: None,
            email_addresses: vec![],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: None,
        };

        let contact = map_google_person(&person);
        assert_eq!(contact.name.display, "Solo");
        assert_eq!(contact.name.given, "Solo");
        assert!(contact.emails.is_empty());
        assert!(contact.phones.is_empty());
        assert!(contact.addresses.is_empty());
    }

    // -------------------------------------------------------------------
    // map_google_person tests — multiple emails, phones, addresses, edge cases
    // -------------------------------------------------------------------

    #[test]
    fn map_google_person_multiple_emails_with_types() {
        let mut person = test_person();
        person.email_addresses = vec![
            GoogleEmailAddress {
                value: "work@example.com".into(),
                email_type: Some("work".into()),
            },
            GoogleEmailAddress {
                value: "home@example.com".into(),
                email_type: Some("home".into()),
            },
            GoogleEmailAddress {
                value: "other@example.com".into(),
                email_type: None,
            },
        ];

        let contact = map_google_person(&person);
        assert_eq!(contact.emails.len(), 3);
        assert_eq!(contact.emails[0].address, "work@example.com");
        assert_eq!(contact.emails[0].email_type.as_deref(), Some("work"));
        assert_eq!(contact.emails[1].address, "home@example.com");
        assert_eq!(contact.emails[1].email_type.as_deref(), Some("home"));
        assert_eq!(contact.emails[2].address, "other@example.com");
        assert!(contact.emails[2].email_type.is_none());
    }

    #[test]
    fn map_google_person_multiple_phones_with_types() {
        let mut person = test_person();
        person.phone_numbers = vec![
            GooglePhoneNumber {
                value: "+1-555-0001".into(),
                phone_type: Some("mobile".into()),
            },
            GooglePhoneNumber {
                value: "+1-555-0002".into(),
                phone_type: Some("work".into()),
            },
            GooglePhoneNumber {
                value: "+1-555-0003".into(),
                phone_type: Some("home".into()),
            },
            GooglePhoneNumber {
                value: "+1-555-0004".into(),
                phone_type: None,
            },
        ];

        let contact = map_google_person(&person);
        assert_eq!(contact.phones.len(), 4);
        assert_eq!(contact.phones[0].number, "+1-555-0001");
        assert_eq!(contact.phones[0].phone_type.as_deref(), Some("mobile"));
        assert_eq!(contact.phones[1].number, "+1-555-0002");
        assert_eq!(contact.phones[1].phone_type.as_deref(), Some("work"));
        assert_eq!(contact.phones[2].number, "+1-555-0003");
        assert_eq!(contact.phones[2].phone_type.as_deref(), Some("home"));
        assert_eq!(contact.phones[3].number, "+1-555-0004");
        assert!(contact.phones[3].phone_type.is_none());
    }

    #[test]
    fn map_google_person_multiple_addresses_all_fields() {
        let mut person = test_person();
        person.addresses = vec![
            GoogleAddress {
                street_address: Some("123 Main St".into()),
                city: Some("Springfield".into()),
                region: Some("IL".into()),
                postal_code: Some("62701".into()),
                country: Some("US".into()),
                address_type: Some("home".into()),
            },
            GoogleAddress {
                street_address: Some("456 Corporate Blvd".into()),
                city: Some("Chicago".into()),
                region: Some("IL".into()),
                postal_code: Some("60601".into()),
                country: Some("US".into()),
                address_type: Some("work".into()),
            },
        ];

        let contact = map_google_person(&person);
        assert_eq!(contact.addresses.len(), 2);

        assert_eq!(contact.addresses[0].street.as_deref(), Some("123 Main St"));
        assert_eq!(contact.addresses[0].city.as_deref(), Some("Springfield"));
        assert_eq!(contact.addresses[0].state.as_deref(), Some("IL"));
        assert_eq!(contact.addresses[0].postcode.as_deref(), Some("62701"));
        assert_eq!(contact.addresses[0].country.as_deref(), Some("US"));

        assert_eq!(
            contact.addresses[1].street.as_deref(),
            Some("456 Corporate Blvd")
        );
        assert_eq!(contact.addresses[1].city.as_deref(), Some("Chicago"));
        assert_eq!(contact.addresses[1].state.as_deref(), Some("IL"));
        assert_eq!(contact.addresses[1].postcode.as_deref(), Some("60601"));
        assert_eq!(contact.addresses[1].country.as_deref(), Some("US"));
    }

    #[test]
    fn map_google_person_empty_resource_name_produces_empty_source_id() {
        let mut person = test_person();
        person.resource_name = String::new();

        let contact = map_google_person(&person);
        assert_eq!(contact.source_id, "");
    }

    #[test]
    fn map_google_person_display_name_only_no_given_family() {
        let person = GooglePerson {
            resource_name: "people/c777".into(),
            display_name: Some("Prince".into()),
            given_name: None,
            family_name: None,
            email_addresses: vec![],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: None,
        };

        let contact = map_google_person(&person);
        assert_eq!(contact.name.display, "Prince");
        assert_eq!(contact.name.given, "");
        assert_eq!(contact.name.family, "");
    }

    #[test]
    fn map_google_person_no_names_at_all_produces_empty_display() {
        let person = GooglePerson {
            resource_name: "people/c888".into(),
            display_name: None,
            given_name: None,
            family_name: None,
            email_addresses: vec![],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: None,
        };

        let contact = map_google_person(&person);
        assert_eq!(contact.name.display, "");
        assert_eq!(contact.name.given, "");
        assert_eq!(contact.name.family, "");
    }

    #[test]
    fn map_google_person_organisation_none_produces_none() {
        let mut person = test_person();
        person.organisation = None;

        let contact = map_google_person(&person);
        assert!(contact.organisation.is_none());
    }

    #[test]
    fn map_google_person_source_always_google() {
        let person = test_person();
        let contact = map_google_person(&person);
        assert_eq!(contact.source, "google");
    }

    #[test]
    fn map_google_person_timestamps_are_set() {
        let before = Utc::now();
        let contact = map_google_person(&test_person());
        let after = Utc::now();

        assert!(
            contact.created_at >= before && contact.created_at <= after,
            "created_at should be between before and after"
        );
        assert!(
            contact.updated_at >= before && contact.updated_at <= after,
            "updated_at should be between before and after"
        );
    }

    #[test]
    fn map_google_person_id_is_non_nil_uuid() {
        let contact = map_google_person(&test_person());
        assert!(!contact.id.is_nil(), "id should be a non-nil UUID");
    }

    #[test]
    fn map_google_person_extensions_always_none() {
        let contact = map_google_person(&test_person());
        assert!(contact.extensions.is_none());
    }

    // -------------------------------------------------------------------
    // Sync lifecycle tests
    // -------------------------------------------------------------------

    #[test]
    fn update_sync_token_sets_both_fields() {
        let mut client = GoogleContactsClient::new(test_config());
        client.update_sync_token("token-1".into());

        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("token-1")
        );
        assert!(
            client.sync_state().last_sync.is_some(),
            "last_sync should be set after update_sync_token"
        );
    }

    #[test]
    fn update_sync_token_last_sync_is_recent() {
        let before = Utc::now();
        let mut client = GoogleContactsClient::new(test_config());
        client.update_sync_token("recent-token".into());
        let after = Utc::now();

        let last_sync = client
            .sync_state()
            .last_sync
            .expect("last_sync should be set");
        assert!(
            last_sync >= before && last_sync <= after,
            "last_sync should be within 1 second of now"
        );
    }

    #[test]
    fn update_sync_token_twice_overwrites_token_advances_last_sync() {
        let mut client = GoogleContactsClient::new(test_config());

        client.update_sync_token("first-token".into());
        let first_sync = client
            .sync_state()
            .last_sync
            .expect("last_sync should be set after first update");

        // Ensure a tiny delay so timestamps can differ
        // (in practice Utc::now() should advance even without sleep)
        client.update_sync_token("second-token".into());
        let second_sync = client
            .sync_state()
            .last_sync
            .expect("last_sync should be set after second update");

        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("second-token"),
            "sync_token should be overwritten to second-token"
        );
        assert!(
            second_sync >= first_sync,
            "second last_sync should be >= first last_sync"
        );
    }

    #[test]
    fn reset_sync_state_clears_both_fields() {
        let mut client = GoogleContactsClient::new(test_config());
        client.update_sync_token("some-token".into());
        assert!(client.sync_state().sync_token.is_some());
        assert!(client.sync_state().last_sync.is_some());

        client.reset_sync_state();
        assert!(
            client.sync_state().sync_token.is_none(),
            "sync_token should be None after reset"
        );
        assert!(
            client.sync_state().last_sync.is_none(),
            "last_sync should be None after reset"
        );
    }

    #[test]
    fn google_sync_state_default_has_all_fields_none() {
        let state = GoogleSyncState::default();
        assert!(state.sync_token.is_none());
        assert!(state.last_sync.is_none());
    }

    // -------------------------------------------------------------------
    // Deserialization tests
    // -------------------------------------------------------------------

    #[test]
    fn deserialize_google_person_missing_optional_fields() {
        let json = r#"{
            "resource_name": "people/c42"
        }"#;
        let person: GooglePerson =
            serde_json::from_str(json).expect("should deserialize with missing optional fields");
        assert_eq!(person.resource_name, "people/c42");
        assert!(person.display_name.is_none());
        assert!(person.given_name.is_none());
        assert!(person.family_name.is_none());
        assert!(person.email_addresses.is_empty());
        assert!(person.phone_numbers.is_empty());
        assert!(person.addresses.is_empty());
        assert!(person.organisation.is_none());
        assert!(person.etag.is_none());
    }

    #[test]
    fn deserialize_realistic_google_people_api_response() {
        let json = r#"{
            "connections": [
                {
                    "resource_name": "people/c1001",
                    "display_name": "Alice Wonderland",
                    "given_name": "Alice",
                    "family_name": "Wonderland",
                    "email_addresses": [
                        {"value": "alice@wonder.land", "type": "home"},
                        {"value": "a.wonderland@corp.com", "type": "work"}
                    ],
                    "phone_numbers": [
                        {"value": "+44 7700 900000", "type": "mobile"}
                    ],
                    "addresses": [
                        {
                            "street_address": "1 Rabbit Hole Lane",
                            "city": "Oxford",
                            "region": "Oxfordshire",
                            "postal_code": "OX1 1AA",
                            "country": "GB",
                            "type": "home"
                        }
                    ],
                    "organisation": "Wonderland Inc",
                    "etag": "etag-abc-123"
                },
                {
                    "resource_name": "people/c1002",
                    "display_name": "Bob Builder"
                }
            ],
            "nextSyncToken": "realistic-sync-token-xyz",
            "totalPeople": 2
        }"#;

        let response: GoogleListResponse =
            serde_json::from_str(json).expect("should deserialize realistic API response");
        assert_eq!(response.connections.len(), 2);

        let alice = &response.connections[0];
        assert_eq!(alice.resource_name, "people/c1001");
        assert_eq!(alice.display_name.as_deref(), Some("Alice Wonderland"));
        assert_eq!(alice.email_addresses.len(), 2);
        assert_eq!(alice.email_addresses[0].value, "alice@wonder.land");
        assert_eq!(alice.email_addresses[0].email_type.as_deref(), Some("home"));
        assert_eq!(alice.email_addresses[1].value, "a.wonderland@corp.com");
        assert_eq!(alice.email_addresses[1].email_type.as_deref(), Some("work"));
        assert_eq!(alice.phone_numbers.len(), 1);
        assert_eq!(alice.phone_numbers[0].value, "+44 7700 900000");
        assert_eq!(alice.addresses.len(), 1);
        assert_eq!(alice.addresses[0].city.as_deref(), Some("Oxford"));
        assert_eq!(alice.organisation.as_deref(), Some("Wonderland Inc"));
        assert_eq!(alice.etag.as_deref(), Some("etag-abc-123"));

        let bob = &response.connections[1];
        assert_eq!(bob.resource_name, "people/c1002");
        assert_eq!(bob.display_name.as_deref(), Some("Bob Builder"));
        assert!(bob.email_addresses.is_empty());
        assert!(bob.phone_numbers.is_empty());
        assert!(bob.addresses.is_empty());

        assert_eq!(
            response.next_sync_token.as_deref(),
            Some("realistic-sync-token-xyz")
        );
        assert!(response.next_page_token.is_none());
        assert_eq!(response.total_people, Some(2));
    }

    #[test]
    fn roundtrip_google_person_serialization() {
        let person = test_person();
        let json = serde_json::to_string(&person).expect("serialize test_person");
        let restored: GooglePerson =
            serde_json::from_str(&json).expect("deserialize test_person roundtrip");

        assert_eq!(restored.resource_name, person.resource_name);
        assert_eq!(restored.display_name, person.display_name);
        assert_eq!(restored.given_name, person.given_name);
        assert_eq!(restored.family_name, person.family_name);
        assert_eq!(restored.email_addresses.len(), person.email_addresses.len());
        assert_eq!(restored.phone_numbers.len(), person.phone_numbers.len());
        assert_eq!(restored.addresses.len(), person.addresses.len());
        assert_eq!(restored.organisation, person.organisation);
        assert_eq!(restored.etag, person.etag);
    }

    #[test]
    fn roundtrip_google_sync_state_serialization() {
        let state = GoogleSyncState {
            sync_token: Some("token-abc".into()),
            last_sync: Some(Utc::now()),
        };
        let json = serde_json::to_string(&state).expect("serialize GoogleSyncState");
        let restored: GoogleSyncState =
            serde_json::from_str(&json).expect("deserialize GoogleSyncState roundtrip");
        assert_eq!(restored.sync_token, state.sync_token);
        assert!(restored.last_sync.is_some());
    }

    #[test]
    fn roundtrip_google_config_serialization() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize config");
        let restored: GoogleContactsConfig =
            serde_json::from_str(&json).expect("deserialize config roundtrip");
        assert_eq!(restored.client_id, config.client_id);
        assert_eq!(restored.client_secret_key, config.client_secret_key);
        assert_eq!(restored.refresh_token_key, config.refresh_token_key);
    }

    #[test]
    fn google_list_response_deserialization_full() {
        let json = r#"{
            "connections": [
                {
                    "resource_name": "people/c111",
                    "display_name": "Alice",
                    "given_name": "Alice",
                    "family_name": "Smith",
                    "email_addresses": [],
                    "phone_numbers": [],
                    "addresses": []
                }
            ],
            "nextPageToken": "page-2-token",
            "nextSyncToken": "sync-token-abc",
            "totalPeople": 42
        }"#;

        let response: GoogleListResponse =
            serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.connections.len(), 1);
        assert_eq!(response.connections[0].resource_name, "people/c111");
        assert_eq!(
            response.next_page_token.as_deref(),
            Some("page-2-token")
        );
        assert_eq!(
            response.next_sync_token.as_deref(),
            Some("sync-token-abc")
        );
        assert_eq!(response.total_people, Some(42));
    }

    #[test]
    fn google_list_response_deserialization_empty() {
        let json = r#"{ "connections": [] }"#;
        let response: GoogleListResponse =
            serde_json::from_str(json).expect("deserialize");
        assert!(response.connections.is_empty());
        assert!(response.next_page_token.is_none());
        assert!(response.next_sync_token.is_none());
        assert!(response.total_people.is_none());
    }

    #[test]
    fn google_list_response_deserialization_no_connections_key() {
        let json = r#"{ "nextSyncToken": "sync-123" }"#;
        let response: GoogleListResponse =
            serde_json::from_str(json).expect("deserialize");
        assert!(response.connections.is_empty());
        assert_eq!(
            response.next_sync_token.as_deref(),
            Some("sync-123")
        );
    }

    #[test]
    fn google_list_response_serialization_roundtrip() {
        let response = GoogleListResponse {
            connections: vec![GooglePerson {
                resource_name: "people/c222".into(),
                display_name: Some("Bob".into()),
                given_name: Some("Bob".into()),
                family_name: Some("Jones".into()),
                email_addresses: vec![],
                phone_numbers: vec![],
                addresses: vec![],
                organisation: None,
                etag: None,
            }],
            next_page_token: Some("page-token".into()),
            next_sync_token: Some("sync-token".into()),
            total_people: Some(1),
        };

        let json = serde_json::to_string(&response).expect("serialize");
        let restored: GoogleListResponse =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.connections.len(), 1);
        assert_eq!(
            restored.next_page_token.as_deref(),
            Some("page-token")
        );
        assert_eq!(
            restored.next_sync_token.as_deref(),
            Some("sync-token")
        );
    }

    #[test]
    fn process_list_response_maps_contacts() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });

        let response = GoogleListResponse {
            connections: vec![
                GooglePerson {
                    resource_name: "people/c100".into(),
                    display_name: Some("Alice".into()),
                    given_name: Some("Alice".into()),
                    family_name: Some("Smith".into()),
                    email_addresses: vec![GoogleEmailAddress {
                        value: "alice@example.com".into(),
                        email_type: Some("work".into()),
                    }],
                    phone_numbers: vec![],
                    addresses: vec![],
                    organisation: None,
                    etag: None,
                },
                GooglePerson {
                    resource_name: "people/c200".into(),
                    display_name: Some("Bob".into()),
                    given_name: Some("Bob".into()),
                    family_name: Some("Jones".into()),
                    email_addresses: vec![],
                    phone_numbers: vec![],
                    addresses: vec![],
                    organisation: None,
                    etag: None,
                },
            ],
            next_page_token: None,
            next_sync_token: Some("sync-after-full".into()),
            total_people: Some(2),
        };

        let contacts = client.process_list_response(response);
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].name.display, "Alice");
        assert_eq!(contacts[0].source, "google");
        assert_eq!(contacts[0].source_id, "people/c100");
        assert_eq!(contacts[1].name.display, "Bob");

        // Sync token should be updated
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("sync-after-full")
        );
        assert!(client.sync_state().last_sync.is_some());
    }

    #[test]
    fn process_list_response_empty_connections() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });

        let response = GoogleListResponse {
            connections: vec![],
            next_page_token: None,
            next_sync_token: Some("sync-empty".into()),
            total_people: Some(0),
        };

        let contacts = client.process_list_response(response);
        assert!(contacts.is_empty());
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("sync-empty")
        );
    }

    #[test]
    fn process_list_response_pagination_has_more() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });

        let response = GoogleListResponse {
            connections: vec![GooglePerson {
                resource_name: "people/c300".into(),
                display_name: Some("Charlie".into()),
                given_name: Some("Charlie".into()),
                family_name: None,
                email_addresses: vec![],
                phone_numbers: vec![],
                addresses: vec![],
                organisation: None,
                etag: None,
            }],
            next_page_token: Some("page-2".into()),
            next_sync_token: None,
            total_people: Some(100),
        };

        let contacts = client.process_list_response(response);
        assert_eq!(contacts.len(), 1);
        // No sync token in paginated response — sync token only on final page
        assert!(client.sync_state().sync_token.is_none());
    }

    #[test]
    fn sync_token_lifecycle() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });

        // Initial state: no sync token
        assert!(client.sync_state().sync_token.is_none());

        // Simulate full sync response with sync token
        let full_response = GoogleListResponse {
            connections: vec![GooglePerson {
                resource_name: "people/c1".into(),
                display_name: Some("First".into()),
                given_name: Some("First".into()),
                family_name: None,
                email_addresses: vec![],
                phone_numbers: vec![],
                addresses: vec![],
                organisation: None,
                etag: None,
            }],
            next_page_token: None,
            next_sync_token: Some("initial-sync-token".into()),
            total_people: Some(1),
        };

        let contacts = client.process_list_response(full_response);
        assert_eq!(contacts.len(), 1);
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("initial-sync-token")
        );

        // Simulate incremental sync response with new sync token
        let incremental_response = GoogleListResponse {
            connections: vec![GooglePerson {
                resource_name: "people/c2".into(),
                display_name: Some("Second".into()),
                given_name: Some("Second".into()),
                family_name: None,
                email_addresses: vec![],
                phone_numbers: vec![],
                addresses: vec![],
                organisation: None,
                etag: None,
            }],
            next_page_token: None,
            next_sync_token: Some("updated-sync-token".into()),
            total_people: Some(1),
        };

        let contacts = client.process_list_response(incremental_response);
        assert_eq!(contacts.len(), 1);
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("updated-sync-token")
        );
    }

    #[test]
    fn google_person_serialization() {
        let person = GooglePerson {
            resource_name: "people/c12345".into(),
            display_name: Some("Test Person".into()),
            given_name: Some("Test".into()),
            family_name: Some("Person".into()),
            email_addresses: vec![GoogleEmailAddress {
                value: "test@example.com".into(),
                email_type: Some("home".into()),
            }],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: None,
        };

        let json = serde_json::to_string(&person).expect("serialize");
        let restored: GooglePerson = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.resource_name, "people/c12345");
        assert_eq!(restored.display_name.as_deref(), Some("Test Person"));
        assert_eq!(restored.email_addresses.len(), 1);
    }

    // -------------------------------------------------------------------
    // OAuthTokenCache tests
    // -------------------------------------------------------------------

    #[test]
    fn token_cache_not_expired() {
        let cache = OAuthTokenCache {
            access_token: "test-token".into(),
            expires_at: Utc::now() + Duration::seconds(3600),
        };
        assert!(!cache.is_expired());
    }

    #[test]
    fn token_cache_expired() {
        let cache = OAuthTokenCache {
            access_token: "test-token".into(),
            expires_at: Utc::now() - Duration::seconds(10),
        };
        assert!(cache.is_expired());
    }

    #[test]
    fn token_cache_expired_within_margin() {
        // Token expires in 30 seconds, but 60-second margin means it is treated as expired
        let cache = OAuthTokenCache {
            access_token: "test-token".into(),
            expires_at: Utc::now() + Duration::seconds(30),
        };
        assert!(cache.is_expired());
    }

    #[test]
    fn client_has_http_client_and_no_cached_token() {
        let client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });
        assert!(client.token_cache().is_none());
    }

    // -------------------------------------------------------------------
    // ensure_valid_token tests (using mockito)
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn ensure_valid_token_returns_cached_when_valid() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });

        // Pre-populate a valid cached token
        client.token_cache = Some(OAuthTokenCache {
            access_token: "cached-access-token".into(),
            expires_at: Utc::now() + Duration::seconds(3600),
        });

        let token = client
            .ensure_valid_token("secret", "token")
            .await
            .expect("should succeed");
        assert_eq!(token, "cached-access-token");
    }

    #[tokio::test]
    async fn ensure_valid_token_refreshes_when_expired() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "access_token": "new-access-token",
                    "expires_in": 3600,
                    "token_type": "Bearer"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = GoogleContactsClient::with_endpoints(
            GoogleContactsConfig {
                client_id: "id".into(),
                client_secret_key: "google_contacts_client_secret".into(),
                refresh_token_key: "google_contacts_refresh_token".into(),
            },
            format!("{}/v1", server.url()),
            format!("{}/token", server.url()),
        );

        // Pre-populate an expired cached token
        client.token_cache = Some(OAuthTokenCache {
            access_token: "old-token".into(),
            expires_at: Utc::now() - Duration::seconds(100),
        });

        let token = client
            .ensure_valid_token("secret", "refresh")
            .await
            .expect("should refresh successfully");
        assert_eq!(token, "new-access-token");

        // Verify the cache was updated
        assert!(!client.token_cache().unwrap().is_expired());
        assert_eq!(
            client.token_cache().unwrap().access_token,
            "new-access-token"
        );
    }

    #[tokio::test]
    async fn ensure_valid_token_refreshes_when_no_cache() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "access_token": "fresh-token",
                    "expires_in": 3600,
                    "token_type": "Bearer"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = GoogleContactsClient::with_endpoints(
            GoogleContactsConfig {
                client_id: "id".into(),
                client_secret_key: "google_contacts_client_secret".into(),
                refresh_token_key: "google_contacts_refresh_token".into(),
            },
            format!("{}/v1", server.url()),
            format!("{}/token", server.url()),
        );

        assert!(client.token_cache().is_none());

        let token = client
            .ensure_valid_token("secret", "refresh")
            .await
            .expect("should refresh successfully");
        assert_eq!(token, "fresh-token");
    }

    #[tokio::test]
    async fn ensure_valid_token_errors_on_refresh_failure() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/token")
            .with_status(401)
            .with_body(r#"{"error": "invalid_grant"}"#)
            .create_async()
            .await;

        let mut client = GoogleContactsClient::with_endpoints(
            GoogleContactsConfig {
                client_id: "id".into(),
                client_secret_key: "google_contacts_client_secret".into(),
                refresh_token_key: "google_contacts_refresh_token".into(),
            },
            format!("{}/v1", server.url()),
            format!("{}/token", server.url()),
        );

        let result = client.ensure_valid_token("secret", "bad-refresh").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    // -------------------------------------------------------------------
    // API method tests (using mockito mock server)
    // -------------------------------------------------------------------

    /// Helper to create a client with endpoints pointing at a mock server
    /// and a pre-populated valid token cache.
    fn mock_client(server_url: &str) -> GoogleContactsClient {
        let mut client = GoogleContactsClient::with_endpoints(
            GoogleContactsConfig {
                client_id: "id".into(),
                client_secret_key: "google_contacts_client_secret".into(),
                refresh_token_key: "google_contacts_refresh_token".into(),
            },
            format!("{}/v1", server_url),
            format!("{}/token", server_url),
        );
        client.token_cache = Some(OAuthTokenCache {
            access_token: "valid-token".into(),
            expires_at: Utc::now() + Duration::seconds(3600),
        });
        client
    }

    #[tokio::test]
    async fn list_connections_single_page() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "connections": [
                        {
                            "resource_name": "people/c111",
                            "display_name": "Alice",
                            "given_name": "Alice",
                            "family_name": "Smith",
                            "email_addresses": [],
                            "phone_numbers": [],
                            "addresses": []
                        }
                    ],
                    "nextSyncToken": "sync-token-abc"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let persons = client
            .list_connections("secret", "refresh")
            .await
            .expect("should succeed");

        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].resource_name, "people/c111");
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("sync-token-abc")
        );
    }

    #[tokio::test]
    async fn list_connections_handles_pagination() {
        let mut server = mockito::Server::new_async().await;

        // Mockito serves mocks in LIFO order unless using `expect`. We use
        // expect(1) on each to ensure ordered consumption.

        // First page response (matched first)
        let _page1 = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .expect(1)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "connections": [
                        {
                            "resource_name": "people/c1",
                            "display_name": "First"
                        }
                    ],
                    "nextPageToken": "page-2-token"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Second page response (matched second)
        let _page2 = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .expect(1)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "connections": [
                        {
                            "resource_name": "people/c2",
                            "display_name": "Second"
                        }
                    ],
                    "nextSyncToken": "final-sync-token"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let persons = client
            .list_connections("secret", "refresh")
            .await
            .expect("should succeed");

        assert_eq!(persons.len(), 2);
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("final-sync-token")
        );
    }

    #[tokio::test]
    async fn list_connections_handles_410_gone_fallback() {
        let mut server = mockito::Server::new_async().await;

        // First call returns 410 Gone (expired sync token)
        let _gone = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .expect(1)
            .with_status(410)
            .with_body("sync token expired")
            .create_async()
            .await;

        // Retry without sync token should succeed
        let _full = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .expect(1)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "connections": [
                        {
                            "resource_name": "people/c1",
                            "display_name": "Full Sync"
                        }
                    ],
                    "nextSyncToken": "fresh-sync-token"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        client.update_sync_token("old-token".into());

        let persons = client
            .list_connections("secret", "refresh")
            .await
            .expect("should succeed after fallback");

        assert_eq!(persons.len(), 1);
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("fresh-sync-token")
        );
    }

    #[tokio::test]
    async fn list_connections_returns_error_on_non_410_failure() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .with_status(500)
            .with_body("internal server error")
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let result = client.list_connections("secret", "refresh").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn get_contact_success() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/c12345.*".into()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "resource_name": "people/c12345",
                    "display_name": "Found Person",
                    "given_name": "Found",
                    "family_name": "Person"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let person = client
            .get_contact("people/c12345", "secret", "refresh")
            .await
            .expect("should succeed");

        assert_eq!(person.resource_name, "people/c12345");
        assert_eq!(person.display_name.as_deref(), Some("Found Person"));
    }

    #[tokio::test]
    async fn get_contact_not_found() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/c99999.*".into()),
            )
            .with_status(404)
            .with_body("not found")
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let result = client.get_contact("people/c99999", "secret", "refresh").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));
    }

    #[tokio::test]
    async fn create_contact_success() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "POST",
                mockito::Matcher::Regex(r"/v1/people:createContact.*".into()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "resource_name": "people/c_new_123",
                    "display_name": "New Person",
                    "given_name": "New",
                    "family_name": "Person",
                    "etag": "create-etag"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let person = GooglePerson {
            resource_name: String::new(),
            display_name: Some("New Person".into()),
            given_name: Some("New".into()),
            family_name: Some("Person".into()),
            email_addresses: vec![],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: None,
        };

        let created = client
            .create_contact(&person, "secret", "refresh")
            .await
            .expect("should succeed");

        assert_eq!(created.resource_name, "people/c_new_123");
        assert_eq!(created.etag.as_deref(), Some("create-etag"));
    }

    #[tokio::test]
    async fn update_contact_success() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "PATCH",
                mockito::Matcher::Regex(r"/v1/people/c555:updateContact.*".into()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "resource_name": "people/c555",
                    "display_name": "Updated Person",
                    "given_name": "Updated",
                    "family_name": "Person",
                    "etag": "updated-etag"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let person = GooglePerson {
            resource_name: "people/c555".into(),
            display_name: Some("Updated Person".into()),
            given_name: Some("Updated".into()),
            family_name: Some("Person".into()),
            email_addresses: vec![],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: Some("old-etag".into()),
        };

        let updated = client
            .update_contact(&person, "secret", "refresh")
            .await
            .expect("should succeed");

        assert_eq!(updated.resource_name, "people/c555");
        assert_eq!(updated.etag.as_deref(), Some("updated-etag"));
    }

    #[tokio::test]
    async fn delete_contact_success() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "DELETE",
                mockito::Matcher::Regex(r"/v1/people/c777:deleteContact.*".into()),
            )
            .with_status(200)
            .with_body("")
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        client
            .delete_contact("people/c777", "secret", "refresh")
            .await
            .expect("should succeed");
    }

    #[tokio::test]
    async fn delete_contact_not_found() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "DELETE",
                mockito::Matcher::Regex(r"/v1/people/c888:deleteContact.*".into()),
            )
            .with_status(404)
            .with_body("not found")
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let result = client.delete_contact("people/c888", "secret", "refresh").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));
    }

    #[tokio::test]
    async fn sync_contacts_returns_cdm_contacts() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/v1/people/me/connections.*".into()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "connections": [
                        {
                            "resource_name": "people/c789",
                            "display_name": "Sync Test",
                            "given_name": "Sync",
                            "family_name": "Test",
                            "email_addresses": [
                                {"value": "sync@example.com"}
                            ]
                        }
                    ],
                    "nextSyncToken": "sync-after-test"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut client = mock_client(&server.url());
        let contacts = client
            .sync_contacts("secret", "refresh")
            .await
            .expect("should succeed");

        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name.display, "Sync Test");
        assert_eq!(contacts[0].source, "google");
        assert_eq!(contacts[0].emails.len(), 1);
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("sync-after-test")
        );
    }

    #[tokio::test]
    async fn sync_token_cleared_on_reset_for_full_resync() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });
        client.update_sync_token("old-token".into());
        assert!(client.sync_state().sync_token.is_some());

        client.reset_sync_state();
        assert!(client.sync_state().sync_token.is_none());
        assert!(client.sync_state().last_sync.is_none());
    }

    #[test]
    fn google_person_with_etag_serialization() {
        let person = GooglePerson {
            resource_name: "people/c555".into(),
            display_name: Some("Etag Person".into()),
            given_name: Some("Etag".into()),
            family_name: Some("Person".into()),
            email_addresses: vec![],
            phone_numbers: vec![],
            addresses: vec![],
            organisation: None,
            etag: Some("abc123etag".into()),
        };

        let json = serde_json::to_string(&person).expect("serialize");
        let restored: GooglePerson =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.etag.as_deref(), Some("abc123etag"));
    }

    #[test]
    fn google_person_etag_defaults_to_none() {
        let json = r#"{
            "resource_name": "people/c666",
            "display_name": "No Etag"
        }"#;
        let person: GooglePerson =
            serde_json::from_str(json).expect("deserialize");
        assert!(person.etag.is_none());
    }

    #[test]
    fn process_list_response_updates_sync_token() {
        let mut client = GoogleContactsClient::new(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });

        let response = GoogleListResponse {
            connections: vec![GooglePerson {
                resource_name: "people/c789".into(),
                display_name: Some("Sync Test".into()),
                given_name: Some("Sync".into()),
                family_name: Some("Test".into()),
                email_addresses: vec![GoogleEmailAddress {
                    value: "sync@example.com".into(),
                    email_type: None,
                }],
                phone_numbers: vec![],
                addresses: vec![],
                organisation: None,
                etag: None,
            }],
            next_page_token: None,
            next_sync_token: Some("sync-after-test".into()),
            total_people: Some(1),
        };

        let contacts = client.process_list_response(response);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name.display, "Sync Test");
        assert_eq!(contacts[0].source, "google");
        assert_eq!(contacts[0].emails.len(), 1);
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("sync-after-test")
        );
    }

    #[test]
    fn token_response_deserialization() {
        let json = r#"{
            "access_token": "ya29.xxx",
            "expires_in": 3600,
            "token_type": "Bearer"
        }"#;
        let resp: TokenResponse =
            serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.access_token, "ya29.xxx");
        assert_eq!(resp.expires_in, 3600);
    }

    #[test]
    fn constants_are_correct() {
        assert!(PEOPLE_API_BASE.starts_with("https://people.googleapis.com"));
        assert!(TOKEN_ENDPOINT.starts_with("https://oauth2.googleapis.com"));
        assert!(PERSON_FIELDS.contains("names"));
        assert!(PERSON_FIELDS.contains("emailAddresses"));
        assert!(PERSON_FIELDS.contains("phoneNumbers"));
        assert!(PERSON_FIELDS.contains("addresses"));
        assert!(PERSON_FIELDS.contains("organizations"));
    }

    #[test]
    fn with_endpoints_sets_custom_urls() {
        let client = GoogleContactsClient::with_endpoints(
            GoogleContactsConfig {
                client_id: "id".into(),
                client_secret_key: "google_contacts_client_secret".into(),
                refresh_token_key: "google_contacts_refresh_token".into(),
            },
            "http://localhost:9999/v1".into(),
            "http://localhost:9999/token".into(),
        );
        assert_eq!(client.api_base_url, "http://localhost:9999/v1");
        assert_eq!(client.token_endpoint_url, "http://localhost:9999/token");
    }
}
