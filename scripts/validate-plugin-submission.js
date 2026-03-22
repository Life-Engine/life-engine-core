// Plugin submission CI validation script.
// Validates that plugin registry entries meet manifest, size, and capability requirements.

const REVERSE_DOMAIN_RE = /^[a-z][a-z0-9-]*(\.[a-z][a-z0-9-]*)+$/;
const SEMVER_RE = /^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$/;

const BUNDLE_WARN_SIZE = 200 * 1024;   // 200KB
const BUNDLE_REJECT_SIZE = 2 * 1024 * 1024; // 2MB

const VALID_CATEGORIES = [
  'productivity',
  'communication',
  'utilities',
  'finance',
  'health',
  'developer-tools',
  'integrations',
  'other',
];

const KNOWN_CAPABILITIES = new Set([
  'data:read:*', 'data:read:tasks', 'data:read:contacts', 'data:read:emails',
  'data:read:events', 'data:read:notes', 'data:read:files', 'data:read:credentials',
  'data:write:*', 'data:write:tasks', 'data:write:contacts', 'data:write:emails',
  'data:write:events', 'data:write:notes', 'data:write:files', 'data:write:credentials',
  'http:fetch', 'storage:local', 'settings:read', 'settings:write',
  'ui:toast', 'ui:modal', 'ui:navigate', 'ipc:send', 'ipc:receive',
]);

const REQUIRED_FIELDS = ['id', 'name', 'version', 'entry', 'element', 'minShellVersion', 'capabilities', 'category', 'repository'];

/**
 * Validate a plugin submission entry for the plugin registry.
 * @param {Record<string, unknown>} entry - The plugin registry entry to validate
 * @param {Array<{id: string}>} [existingEntries] - Existing registry entries for duplicate detection
 * @returns {{ valid: boolean, errors: string[], warnings: string[] }}
 */
export function validatePluginSubmission(entry, existingEntries = []) {
  /** @type {string[]} */
  const errors = [];
  /** @type {string[]} */
  const warnings = [];

  if (!entry || typeof entry !== 'object') {
    return { valid: false, errors: ['Entry must be an object'], warnings };
  }

  // Check required fields
  for (const field of REQUIRED_FIELDS) {
    if (entry[field] === undefined || entry[field] === null) {
      errors.push(`Missing required field: ${field}`);
    }
  }

  if (errors.length > 0) {
    return { valid: false, errors, warnings };
  }

  // id must be reverse-domain format
  if (typeof entry.id !== 'string' || !REVERSE_DOMAIN_RE.test(entry.id)) {
    errors.push('id must be in reverse-domain format (e.g. com.example.plugin-name)');
  }

  // name must be a non-empty string
  if (typeof entry.name !== 'string' || entry.name.trim() === '') {
    errors.push('name must be a non-empty string');
  }

  // version must be valid semver
  if (typeof entry.version !== 'string' || !SEMVER_RE.test(entry.version)) {
    errors.push('version must be valid semver (e.g. 1.0.0)');
  }

  // entry must end in .js
  if (typeof entry.entry !== 'string' || !entry.entry.endsWith('.js')) {
    errors.push('entry must end in .js');
  }

  // element must contain a hyphen
  if (typeof entry.element !== 'string' || !entry.element.includes('-')) {
    errors.push('element must contain a hyphen (valid custom element name)');
  }

  // minShellVersion must be valid semver
  if (typeof entry.minShellVersion !== 'string' || !SEMVER_RE.test(entry.minShellVersion)) {
    errors.push('minShellVersion must be valid semver');
  }

  // capabilities must be a non-empty array of known strings
  if (!Array.isArray(entry.capabilities)) {
    errors.push('capabilities must be an array');
  } else if (entry.capabilities.length === 0) {
    errors.push('capabilities must not be empty');
  } else {
    const unknown = entry.capabilities.filter((c) => !KNOWN_CAPABILITIES.has(c));
    if (unknown.length > 0) {
      errors.push(`Unknown capabilities: ${unknown.join(', ')}`);
    }
  }

  // category must be one of valid categories
  if (typeof entry.category !== 'string' || !VALID_CATEGORIES.includes(entry.category)) {
    errors.push(`category must be one of: ${VALID_CATEGORIES.join(', ')}`);
  }

  // repository must be a string
  if (typeof entry.repository !== 'string' || entry.repository.trim() === '') {
    errors.push('repository must be a non-empty string URL');
  }

  // Size constraints
  if (entry.bundleSize !== undefined) {
    const size = Number(entry.bundleSize);
    if (size > BUNDLE_REJECT_SIZE) {
      errors.push(`Bundle size ${(size / 1024).toFixed(0)}KB exceeds maximum 2MB`);
    } else if (size > BUNDLE_WARN_SIZE) {
      warnings.push(`Bundle size ${(size / 1024).toFixed(0)}KB exceeds recommended 200KB`);
    }
  }

  // Duplicate detection
  if (existingEntries.length > 0 && entry.id) {
    const isDuplicate = existingEntries.some((e) => e.id === entry.id);
    if (isDuplicate) {
      errors.push(`Plugin with id "${entry.id}" already exists (duplicate)`);
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}
