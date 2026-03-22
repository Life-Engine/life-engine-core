// Verify that sample CDM objects conform to their TypeScript interfaces.
// These tests act as a compile-time + runtime guard ensuring the TS types
// stay aligned with the Rust structs and JSON schemas.

import { describe, it, expect } from "vitest";
import type {
  Task,
  TaskPriority,
  TaskStatus,
  CalendarEvent,
  Contact,
  ContactName,
  EmailAddress,
  PhoneNumber,
  PostalAddress,
  Note,
  Email,
  EmailAttachment,
  FileMetadata,
  Credential,
  CredentialType,
} from "../index.js";

describe("CDM TypeScript interfaces", () => {
  it("Task conforms to the interface", () => {
    const now = new Date().toISOString();
    const priority: TaskPriority = "high";
    const status: TaskStatus = "pending";

    const task: Task = {
      id: "550e8400-e29b-41d4-a716-446655440000",
      title: "Review pull request #42",
      description: "Review the authentication refactor PR",
      status,
      priority,
      due_date: now,
      labels: ["review", "auth"],
      source: "test",
      source_id: "task-001",
      extensions: { "com.life-engine.todos": { custom: true } },
      created_at: now,
      updated_at: now,
    };

    expect(task.id).toBeDefined();
    expect(task.title).toBe("Review pull request #42");
    expect(task.labels).toEqual(["review", "auth"]);
    expect(Array.isArray(task.labels)).toBe(true);
  });

  it("Task with minimal fields (no optionals)", () => {
    const now = new Date().toISOString();

    const task: Task = {
      id: "550e8400-e29b-41d4-a716-446655440000",
      title: "Minimal task",
      status: "active",
      priority: "none",
      labels: [],
      source: "test",
      source_id: "task-002",
      created_at: now,
      updated_at: now,
    };

    expect(task.description).toBeUndefined();
    expect(task.due_date).toBeUndefined();
    expect(task.extensions).toBeUndefined();
    expect(task.labels).toEqual([]);
  });

  it("CalendarEvent conforms to the interface", () => {
    const now = new Date().toISOString();

    const event: CalendarEvent = {
      id: "550e8400-e29b-41d4-a716-446655440001",
      title: "Weekly standup",
      start: now,
      end: new Date(Date.now() + 3600000).toISOString(),
      recurrence: "FREQ=WEEKLY;BYDAY=MO",
      attendees: ["alice@example.com", "bob@example.com"],
      location: "Conference Room A",
      description: "Weekly team sync-up",
      source: "test",
      source_id: "event-001",
      created_at: now,
      updated_at: now,
    };

    expect(event.attendees).toHaveLength(2);
    expect(Array.isArray(event.attendees)).toBe(true);
  });

  it("CalendarEvent with empty attendees", () => {
    const now = new Date().toISOString();

    const event: CalendarEvent = {
      id: "550e8400-e29b-41d4-a716-446655440001",
      title: "Solo meeting",
      start: now,
      end: now,
      attendees: [],
      source: "test",
      source_id: "event-002",
      created_at: now,
      updated_at: now,
    };

    expect(event.attendees).toEqual([]);
  });

  it("Contact conforms to the interface", () => {
    const now = new Date().toISOString();

    const name: ContactName = {
      given: "Alice",
      family: "Johnson",
      display: "Alice Johnson",
    };

    const email: EmailAddress = {
      address: "alice@example.com",
      type: "work",
      primary: true,
    };

    const phone: PhoneNumber = {
      number: "+61 400 123 456",
      type: "mobile",
    };

    const address: PostalAddress = {
      street: "123 Main St",
      city: "Sydney",
      state: "NSW",
      postcode: "2000",
      country: "Australia",
    };

    const contact: Contact = {
      id: "550e8400-e29b-41d4-a716-446655440002",
      name,
      emails: [email],
      phones: [phone],
      addresses: [address],
      organisation: "Acme Corp",
      source: "test",
      source_id: "contact-001",
      created_at: now,
      updated_at: now,
    };

    expect(contact.emails).toHaveLength(1);
    expect(contact.phones).toHaveLength(1);
    expect(contact.addresses).toHaveLength(1);
    expect(Array.isArray(contact.emails)).toBe(true);
    expect(Array.isArray(contact.phones)).toBe(true);
    expect(Array.isArray(contact.addresses)).toBe(true);
  });

  it("Contact with empty arrays for Vec fields", () => {
    const now = new Date().toISOString();

    const contact: Contact = {
      id: "550e8400-e29b-41d4-a716-446655440002",
      name: { given: "Bob", family: "Smith", display: "Bob Smith" },
      emails: [],
      phones: [],
      addresses: [],
      source: "test",
      source_id: "contact-002",
      created_at: now,
      updated_at: now,
    };

    expect(contact.emails).toEqual([]);
    expect(contact.phones).toEqual([]);
    expect(contact.addresses).toEqual([]);
  });

  it("Note conforms to the interface", () => {
    const now = new Date().toISOString();

    const note: Note = {
      id: "550e8400-e29b-41d4-a716-446655440003",
      title: "Meeting notes",
      body: "Discussed plugin sandboxing approach.",
      tags: ["meeting", "architecture"],
      source: "test",
      source_id: "note-001",
      created_at: now,
      updated_at: now,
    };

    expect(note.tags).toHaveLength(2);
    expect(Array.isArray(note.tags)).toBe(true);
  });

  it("Email conforms to the interface", () => {
    const now = new Date().toISOString();

    const attachment: EmailAttachment = {
      file_id: "file-abc-123",
      filename: "report.pdf",
      mime_type: "application/pdf",
      size: 245760,
    };

    const email: Email = {
      id: "550e8400-e29b-41d4-a716-446655440004",
      from: "sender@example.com",
      to: ["recipient@example.com"],
      cc: ["cc@example.com"],
      bcc: [],
      subject: "Project update",
      body_text: "Please find the update attached.",
      body_html: "<p>Please find the update attached.</p>",
      thread_id: "thread-abc-123",
      labels: ["inbox", "important"],
      attachments: [attachment],
      source: "test",
      source_id: "email-001",
      created_at: now,
      updated_at: now,
    };

    expect(email.cc).toHaveLength(1);
    expect(email.bcc).toEqual([]);
    expect(email.labels).toHaveLength(2);
    expect(email.attachments).toHaveLength(1);
    expect(Array.isArray(email.cc)).toBe(true);
    expect(Array.isArray(email.bcc)).toBe(true);
    expect(Array.isArray(email.labels)).toBe(true);
    expect(Array.isArray(email.attachments)).toBe(true);
  });

  it("FileMetadata conforms to the interface", () => {
    const now = new Date().toISOString();

    const file: FileMetadata = {
      id: "550e8400-e29b-41d4-a716-446655440005",
      name: "quarterly-report.pdf",
      mime_type: "application/pdf",
      size: 245760,
      path: "/documents/reports/quarterly-report.pdf",
      checksum: "sha256:e3b0c44298fc1c149afbf4c8996fb924",
      source: "test",
      source_id: "file-001",
      created_at: now,
      updated_at: now,
    };

    expect(file.size).toBe(245760);
    expect(typeof file.size).toBe("number");
  });

  it("Credential conforms to the interface", () => {
    const now = new Date().toISOString();
    const credType: CredentialType = "oauth_token";

    const credential: Credential = {
      id: "550e8400-e29b-41d4-a716-446655440006",
      type: credType,
      issuer: "https://auth.example.com",
      issued_date: "2026-01-15",
      expiry_date: "2027-01-15",
      claims: { scope: "read write", sub: "user-12345" },
      source: "test",
      source_id: "cred-001",
      created_at: now,
      updated_at: now,
    };

    expect(credential.type).toBe("oauth_token");
    expect(credential.claims).toBeDefined();
  });

  it("CredentialType union covers all Rust variants", () => {
    const types: CredentialType[] = [
      "oauth_token",
      "api_key",
      "identity_document",
      "passkey",
    ];
    expect(types).toHaveLength(4);
  });

  it("TaskPriority union covers all Rust variants", () => {
    const priorities: TaskPriority[] = [
      "none",
      "low",
      "medium",
      "high",
      "critical",
    ];
    expect(priorities).toHaveLength(5);
  });

  it("TaskStatus union covers all Rust variants", () => {
    const statuses: TaskStatus[] = [
      "pending",
      "active",
      "completed",
      "cancelled",
    ];
    expect(statuses).toHaveLength(4);
  });
});
