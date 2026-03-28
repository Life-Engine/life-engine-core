<!--
domain: schema-and-validation
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Technical Design — Schema and Validation

## Introduction

This document describes the technical design for schema loading, write-time validation, extension field enforcement, index hint propagation, and schema evolution checks. All validation logic lives in `packages/storage` (the storage orchestration layer), not in individual adapters. Adapters receive pre-validated data.

## Schema Format

All schemas use JSON Schema draft 2020-12. The `jsonschema` crate (Rust) handles validation at runtime.

CDM schemas are shipped as `.json` files inside the SDK package at `packages/plugin-sdk/schemas/`. Plugin schemas are resolved relative to the plugin's root directory.

## Schema Resolution

Schema resolution happens at plugin registration time (during `migrate`), not on every write. Resolved schemas are cached in memory.

Resolution flow:

1. Read the collection's `schema` field from `manifest.toml`
2. If the value starts with `cdm:`, strip the prefix and look up `packages/plugin-sdk/schemas/{name}.schema.json`
3. If the value is a relative path, resolve it against the plugin's root directory
4. If the field is absent, mark the collection as schemaless
5. Parse the JSON Schema and store it in a `SchemaRegistry` keyed by `(plugin_id, collection_name)`

```rust
pub struct SchemaRegistry {
    schemas: HashMap<(PluginId, CollectionName), CompiledSchema>,
}

impl SchemaRegistry {
    pub fn resolve(
        &mut self,
        plugin_id: &PluginId,
        collection: &CollectionName,
        declaration: &CollectionDeclaration,
        plugin_root: &Path,
    ) -> Result<(), SchemaError> {
        let schema = match &declaration.schema {
            Some(reference) if reference.starts_with("cdm:") => {
                let name = reference.strip_prefix("cdm:").unwrap();
                let path = sdk_schemas_dir().join(format!("{name}.schema.json"));
                load_and_compile(&path)?
            }
            Some(relative_path) => {
                let path = plugin_root.join(relative_path);
                load_and_compile(&path)?
            }
            None => return Ok(()), // schemaless
        };
        self.schemas.insert((plugin_id.clone(), collection.clone()), schema);
        Ok(())
    }

    pub fn get(
        &self,
        plugin_id: &PluginId,
        collection: &CollectionName,
    ) -> Option<&CompiledSchema> {
        self.schemas.get(&(plugin_id.clone(), collection.clone()))
    }
}
```

## Collection Declaration

A `CollectionDeclaration` is parsed from `manifest.toml` and carries all metadata for a single collection.

```rust
pub struct CollectionDeclaration {
    pub schema: Option<String>,
    pub access: AccessMode,
    pub indexes: Vec<String>,
    pub strict: bool,
    pub extensions: Vec<String>,
    pub extension_schema: Option<String>,
    pub extension_indexes: Vec<String>,
}

pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}
```

## Validation Pipeline

Validation runs inside `StorageContext` before delegating to the adapter. The sequence matches the spec:

```rust
pub fn validate_write(
    registry: &SchemaRegistry,
    plugin_id: &PluginId,
    collection: &CollectionName,
    declaration: &CollectionDeclaration,
    doc: &mut serde_json::Value,
) -> Result<(), StorageError> {
    // Step 1: Protect system-managed fields
    enforce_base_fields(doc)?;

    // Step 2: Validate body against collection schema
    if let Some(schema) = registry.get(plugin_id, collection) {
        validate_against_schema(schema, doc)?;
    }

    // Step 3: Validate extension fields against extension_schema
    if let Some(ext_schema) = registry.get_extension_schema(plugin_id, collection) {
        let ext_fields = extract_extension_fields(doc, plugin_id);
        validate_against_schema(ext_schema, &ext_fields)?;
    }

    // Step 4: Strict mode check
    if declaration.strict {
        if let Some(schema) = registry.get(plugin_id, collection) {
            reject_unknown_fields(schema, doc)?;
        }
    }

    Ok(())
}
```

## System-Managed Base Fields

`StorageContext` handles base fields before validation:

```rust
fn enforce_base_fields(doc: &mut serde_json::Value) -> Result<(), StorageError> {
    let obj = doc.as_object_mut()
        .ok_or(StorageError::ValidationFailed("document must be an object".into()))?;

    // Always overwrite timestamps
    let now = Utc::now().to_rfc3339();
    obj.insert("updated_at".into(), serde_json::Value::String(now.clone()));

    // On create: set created_at, generate id if missing
    // On update: reject attempts to change id or created_at
    // (caller context determines create vs update)

    Ok(())
}
```

On create operations:

- `created_at` is set to the current timestamp (caller value overwritten)
- `updated_at` is set to the current timestamp (caller value overwritten)
- `id` is generated if not provided; accepted if provided

On update operations:

- `updated_at` is set to the current timestamp (caller value overwritten)
- `id` and `created_at` are immutable; attempts to change them return `StorageError::ValidationFailed`

## Extension Field Namespace Enforcement

Extension fields use the format `ext.{plugin_id}.{field_name}`. Enforcement happens in `StorageContext` before the write reaches the adapter.

```rust
fn enforce_extension_namespace(
    doc: &serde_json::Value,
    caller_plugin_id: &PluginId,
) -> Result<(), StorageError> {
    let obj = doc.as_object().unwrap_or(&serde_json::Map::new());
    for key in obj.keys() {
        if let Some(rest) = key.strip_prefix("ext.") {
            if let Some(owner_id) = rest.split('.').next() {
                if owner_id != caller_plugin_id.as_str() {
                    return Err(StorageError::CapabilityDenied(format!(
                        "plugin '{caller_plugin_id}' cannot write to namespace 'ext.{owner_id}'"
                    )));
                }
            }
        }
    }
    Ok(())
}
```

Read operations return all extension fields without filtering. Any plugin can read any extension field, but only the owning plugin can write to its namespace.

## Index Hint Propagation

Index hints flow from `manifest.toml` through `CollectionDescriptor` to the adapter during `migrate`.

```rust
pub struct CollectionDescriptor {
    pub name: CollectionName,
    pub plugin_id: PluginId,
    pub fields: Vec<FieldDescriptor>,
    pub indexes: Vec<String>,
    pub extension_indexes: Vec<String>,
}
```

The adapter receives the descriptor and checks its own capabilities:

- If `AdapterCapabilities.indexing` is `true`, the adapter creates indexes for each declared path
- If `AdapterCapabilities.indexing` is `false`, the adapter ignores index hints silently

CDM schemas ship default index hints. When a plugin also declares indexes for a CDM collection, the plugin's declarations are merged with the CDM defaults. Plugin declarations take precedence on conflict.

## Strict Mode

Strict mode (`strict = true`) adds an additional validation step after schema validation. It walks the document keys and rejects any field not present in the schema's `properties` (or `patternProperties`).

```rust
fn reject_unknown_fields(
    schema: &CompiledSchema,
    doc: &serde_json::Value,
) -> Result<(), StorageError> {
    let defined_fields = schema.defined_property_names();
    let obj = doc.as_object().unwrap_or(&serde_json::Map::new());
    for key in obj.keys() {
        // Skip system fields and extension fields
        if matches!(key.as_str(), "id" | "created_at" | "updated_at") {
            continue;
        }
        if key.starts_with("ext.") {
            continue;
        }
        if !defined_fields.contains(key.as_str()) {
            return Err(StorageError::ValidationFailed(format!(
                "strict mode: unknown field '{key}'"
            )));
        }
    }
    Ok(())
}
```

System-managed fields (`id`, `created_at`, `updated_at`) and extension fields (`ext.*`) are always exempt from strict mode rejection.

## Schema Evolution

Schema evolution is checked at plugin registration time. When a plugin updates its schema, the system compares the new schema against the previously registered version.

Allowed changes within a major SDK version:

- Adding new optional fields (`required` list unchanged or only appended)
- Adding new enum variants

Disallowed changes (breaking):

- Removing a field from `properties`
- Changing a field's `type`
- Adding a field to the `required` list that was previously optional
- Narrowing enum variants

The evolution check runs a structural diff between the old and new compiled schemas. If a breaking change is detected, the system rejects the plugin update with `SchemaError::BreakingChange` and a message identifying the incompatible field.

## Error Types

Validation-related errors:

- **`StorageError::ValidationFailed(String)`** — Schema validation failure, strict mode rejection, or immutable field violation. The message identifies the field and constraint.
- **`StorageError::CapabilityDenied(String)`** — Extension namespace violation. The message identifies the caller and the target namespace.
- **`SchemaError::NotFound(String)`** — Schema file could not be loaded. The message identifies the path.
- **`SchemaError::ParseFailed(String)`** — Schema file is not valid JSON Schema. The message includes the parse error.
- **`SchemaError::BreakingChange(String)`** — Schema evolution check failed. The message identifies the incompatible field and change type.
