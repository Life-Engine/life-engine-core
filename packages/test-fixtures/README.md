# Test Fixtures

Canonical test data for all 7 Life Engine CDM collections. Used by JSON Schema validation tests, Rust struct round-trip tests, and TypeScript type checks.

## Structure

Each collection has two fixture variants:

- `valid-minimal.json` — Only required fields. Tests the minimum valid document.
- `valid-full.json` — All fields including optional ones and extensions. Tests the maximum valid document.

Single-file fixtures in `fixtures/` are used by the Rust `life-engine-test-fixtures` crate for compile-time embedding via `include_str!`.

## Adding Fixtures

1. Create a JSON file in the appropriate collection directory
2. Ensure all required fields from `docs/schemas/{collection}.schema.json` are present
3. Use deterministic values (fixed UUIDs, fixed timestamps)
4. Validate against the schema before committing

## Naming Convention

- `valid-minimal.json` — minimum valid document
- `valid-full.json` — maximum valid document with all optional fields populated
- `invalid-{reason}.json` — documents expected to fail validation (for negative tests)
