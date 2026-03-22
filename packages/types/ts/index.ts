// Canonical Data Model (CDM) TypeScript interfaces for Life Engine.
//
// These types are the single source of truth for the TypeScript side,
// matching the Rust structs in packages/types/src/ and the JSON schemas
// in docs/schemas/. All CDM interfaces live here; SDK-specific types
// remain in their respective SDK packages.

// -- Tasks --

/** Task priority levels. Matches Rust TaskPriority with #[serde(rename_all = "lowercase")]. */
export type TaskPriority = "none" | "low" | "medium" | "high" | "critical";

/** Task status values. Matches Rust TaskStatus with #[serde(rename_all = "lowercase")]. */
export type TaskStatus = "pending" | "active" | "completed" | "cancelled";

/** A task in the Life Engine canonical data model. */
export interface Task {
  id: string;
  title: string;
  description?: string;
  status: TaskStatus;
  priority: TaskPriority;
  due_date?: string;
  /** Labels for categorisation. Defaults to empty array (Rust: Vec<String> with #[serde(default)]). */
  labels: string[];
  source: string;
  source_id: string;
  /**
   * Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
   * Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
   * is an opaque object owned by that plugin. See ADR-014.
   */
  extensions?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

// -- Events --

/** A calendar event in the Life Engine canonical data model. */
export interface CalendarEvent {
  id: string;
  title: string;
  start: string;
  end: string;
  recurrence?: string;
  /** Attendee email addresses or identifiers. Defaults to empty array (Rust: Vec<String> with #[serde(default)]). */
  attendees: string[];
  location?: string;
  description?: string;
  source: string;
  source_id: string;
  /**
   * Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
   * Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
   * is an opaque object owned by that plugin. See ADR-014.
   */
  extensions?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

// -- Contacts --

/** Structured name for a contact. */
export interface ContactName {
  /** Given/first name. */
  given: string;
  /** Family/last name. */
  family: string;
  /** Full display name as the user prefers it. */
  display: string;
}

/** An email address entry for a contact. */
export interface EmailAddress {
  address: string;
  /** Label such as work, personal, other. Rust field: email_type with #[serde(rename = "type")]. */
  type?: string;
  primary?: boolean;
}

/** A phone number entry for a contact. */
export interface PhoneNumber {
  number: string;
  /** Label such as mobile, work, home. Rust field: phone_type with #[serde(rename = "type")]. */
  type?: string;
}

/** A postal address for a contact. */
export interface PostalAddress {
  street?: string;
  city?: string;
  state?: string;
  postcode?: string;
  country?: string;
}

/** A contact in the Life Engine canonical data model. */
export interface Contact {
  id: string;
  name: ContactName;
  /** Contact email addresses. Defaults to empty array (Rust: Vec<EmailAddress> with #[serde(default)]). */
  emails: EmailAddress[];
  /** Contact phone numbers. Defaults to empty array (Rust: Vec<PhoneNumber> with #[serde(default)]). */
  phones: PhoneNumber[];
  /** Contact postal addresses. Defaults to empty array (Rust: Vec<PostalAddress> with #[serde(default)]). */
  addresses: PostalAddress[];
  organisation?: string;
  source: string;
  source_id: string;
  /**
   * Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
   * Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
   * is an opaque object owned by that plugin. See ADR-014.
   */
  extensions?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

// -- Notes --

/** A note in the Life Engine canonical data model. */
export interface Note {
  id: string;
  title: string;
  body: string;
  /** Tags for categorisation. Defaults to empty array (Rust: Vec<String> with #[serde(default)]). */
  tags: string[];
  source: string;
  source_id: string;
  /**
   * Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
   * Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
   * is an opaque object owned by that plugin. See ADR-014.
   */
  extensions?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

// -- Emails --

/** An email attachment reference. */
export interface EmailAttachment {
  file_id: string;
  filename: string;
  mime_type: string;
  size: number;
}

/** An email message in the Life Engine canonical data model. */
export interface Email {
  id: string;
  from: string;
  to: string[];
  /** CC recipients. Defaults to empty array (Rust: Vec<String> with #[serde(default)]). */
  cc: string[];
  /** BCC recipients. Defaults to empty array (Rust: Vec<String> with #[serde(default)]). */
  bcc: string[];
  subject: string;
  body_text: string;
  body_html?: string;
  thread_id?: string;
  /** Labels or folder assignments. Defaults to empty array (Rust: Vec<String> with #[serde(default)]). */
  labels: string[];
  /** File attachment references. Defaults to empty array (Rust: Vec<EmailAttachment> with #[serde(default)]). */
  attachments: EmailAttachment[];
  source: string;
  source_id: string;
  /**
   * Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
   * Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
   * is an opaque object owned by that plugin. See ADR-014.
   */
  extensions?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

// -- Files --

/** File metadata in the Life Engine canonical data model. */
export interface FileMetadata {
  id: string;
  name: string;
  mime_type: string;
  size: number;
  path: string;
  checksum?: string;
  source: string;
  source_id: string;
  /**
   * Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
   * Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
   * is an opaque object owned by that plugin. See ADR-014.
   */
  extensions?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

// -- Credentials --

/** The type of credential stored. Matches Rust CredentialType with #[serde(rename_all = "snake_case")]. */
export type CredentialType =
  | "oauth_token"
  | "api_key"
  | "identity_document"
  | "passkey";

/** A credential in the Life Engine canonical data model. */
export interface Credential {
  id: string;
  /** Credential type. Rust field: credential_type with #[serde(rename = "type")]. */
  type: CredentialType;
  issuer: string;
  issued_date: string;
  expiry_date?: string;
  claims: Record<string, unknown>;
  source: string;
  source_id: string;
  created_at: string;
  updated_at: string;
}
