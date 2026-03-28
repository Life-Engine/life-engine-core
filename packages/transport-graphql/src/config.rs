//! GraphQL schema generation from plugin manifest schemas.
//!
//! Requirement 9: plugins declare schemas in their manifests, and the GraphQL
//! handler generates queryable types from those declarations. Collections
//! without a declared schema are NOT exposed via GraphQL (they remain
//! accessible through REST generic CRUD).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A plugin manifest's schema declaration for a single collection.
///
/// This is the subset of the plugin manifest that the GraphQL transport needs
/// to generate its schema. The full manifest lives in the plugin system crate;
/// this struct mirrors only the schema-relevant fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSchemaDeclaration {
    /// Collection name (e.g. `"tasks"`, `"contacts"`).
    pub collection: String,
    /// Field name to JSON Schema type mapping.
    /// Only scalar types are supported in v1: `"string"`, `"integer"`,
    /// `"number"`, `"boolean"`.
    pub fields: HashMap<String, String>,
}

/// A generated GraphQL type descriptor for a single collection.
#[derive(Debug, Clone)]
pub struct GeneratedGraphqlType {
    /// The GraphQL type name (PascalCase of the collection name).
    pub type_name: String,
    /// The collection this type maps to.
    pub collection: String,
    /// Fields with their GraphQL scalar type names.
    pub fields: Vec<(String, String)>,
}

/// Generate GraphQL type descriptors from a set of plugin schema declarations.
///
/// Each declared collection becomes a queryable GraphQL type. Collections
/// without a declared schema are omitted (Requirement 9.3).
///
/// Returns a list of `GeneratedGraphqlType` values that can be used to build
/// the runtime `async-graphql` schema.
pub fn generate_schema(declarations: &[PluginSchemaDeclaration]) -> Vec<GeneratedGraphqlType> {
    declarations
        .iter()
        .map(|decl| {
            let type_name = to_pascal_case(&decl.collection);
            let fields = decl
                .fields
                .iter()
                .map(|(name, json_type)| {
                    let gql_type = json_type_to_graphql(json_type);
                    (name.clone(), gql_type)
                })
                .collect();
            GeneratedGraphqlType {
                type_name,
                collection: decl.collection.clone(),
                fields,
            }
        })
        .collect()
}

/// Map a JSON Schema type string to a GraphQL scalar type name.
fn json_type_to_graphql(json_type: &str) -> String {
    match json_type {
        "string" => "String".into(),
        "integer" => "Int".into(),
        "number" => "Float".into(),
        "boolean" => "Boolean".into(),
        other => other.to_string(),
    }
}

/// Convert a snake_case or lowercase collection name to PascalCase.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect()
}
