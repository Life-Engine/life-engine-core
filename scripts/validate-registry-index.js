// Plugin registry index validation script.
// Validates the structure of the plugin-registry.json file.

import { validatePluginSubmission } from './validate-plugin-submission.js';

/**
 * Validate a plugin registry index object.
 * @param {Record<string, unknown>} index - The parsed registry index JSON
 * @returns {{ valid: boolean, errors: string[], warnings: string[] }}
 */
export function validateRegistryIndex(index) {
  /** @type {string[]} */
  const errors = [];
  /** @type {string[]} */
  const warnings = [];

  if (!index || typeof index !== 'object') {
    return { valid: false, errors: ['Registry index must be an object'], warnings };
  }

  // version field
  if (index.version === undefined || index.version === null) {
    errors.push('Missing required field: version');
  }

  // updated field
  if (!index.updated || typeof index.updated !== 'string') {
    errors.push('Missing required field: updated (ISO 8601 timestamp)');
  }

  // plugins field
  if (!Array.isArray(index.plugins)) {
    errors.push('Missing required field: plugins (must be an array)');
    return { valid: false, errors, warnings };
  }

  // Check for duplicate IDs
  const seenIds = new Set();
  for (const plugin of index.plugins) {
    if (plugin && plugin.id) {
      if (seenIds.has(plugin.id)) {
        errors.push(`Duplicate plugin id: "${plugin.id}" (duplicate)`);
      }
      seenIds.add(plugin.id);
    }
  }

  // Check alphabetical sorting by id
  for (let i = 1; i < index.plugins.length; i++) {
    const prev = index.plugins[i - 1];
    const curr = index.plugins[i];
    if (prev?.id && curr?.id && prev.id > curr.id) {
      errors.push(`Plugins must be sorted alphabetically by id: "${prev.id}" should come after "${curr.id}" (sorted)`);
    }
  }

  // Validate each entry
  for (let i = 0; i < index.plugins.length; i++) {
    const entry = index.plugins[i];
    const result = validatePluginSubmission(entry);
    for (const err of result.errors) {
      errors.push(`plugins[${i}] (${entry?.id || 'unknown'}): ${err}`);
    }
    for (const warn of result.warnings) {
      warnings.push(`plugins[${i}] (${entry?.id || 'unknown'}): ${warn}`);
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}
