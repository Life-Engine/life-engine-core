//! GraphQL schema generation from plugin manifest schemas.
//!
//! Requirement 9: plugins declare schemas in their manifests, and the GraphQL
//! handler generates queryable types from those declarations. Collections
//! without a declared schema are NOT exposed via GraphQL (they remain
//! accessible through REST generic CRUD).

use std::collections::HashMap;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, Object, Schema, TypeRef};
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

/// Build a runtime `async-graphql` dynamic schema from plugin schema
/// declarations (Requirement 9.1, 9.2, 9.4).
///
/// For each declared collection, generates:
/// - A GraphQL object type with fields matching the schema's properties
/// - Query resolvers: `<collection>_list` and `<collection>_get(id)`
/// - Mutation resolvers: `<collection>_create`, `<collection>_update(id)`,
///   `<collection>_delete(id)`
///
/// Resolvers return placeholder values until the workflow dispatcher is
/// wired in Phase 10.
pub fn build_dynamic_schema(declarations: &[PluginSchemaDeclaration]) -> Schema {
    let types = generate_schema(declarations);

    let mut query = Object::new("Query");
    let mut mutation = Object::new("Mutation");
    let mut schema_builder = Schema::build("Query", Some("Mutation"), None);

    for gql_type in &types {
        // Build the collection object type with its fields.
        let mut object = Object::new(&gql_type.type_name);
        for (field_name, field_type) in &gql_type.fields {
            let type_ref = scalar_type_ref(field_type);
            object = object.field(Field::new(field_name, type_ref, |ctx| {
                FieldFuture::new(async move {
                    let field_name = ctx.field().name().to_string();
                    Ok(Some(FieldValue::value(format!("placeholder:{field_name}"))))
                })
            }));
        }
        schema_builder = schema_builder.register(object);

        // Query: list resolver — returns a list of the collection type.
        let list_field_name = format!("{}_list", gql_type.collection);
        let type_name = gql_type.type_name.clone();
        query = query.field(Field::new(
            &list_field_name,
            TypeRef::named_nn_list(&type_name),
            |_ctx| FieldFuture::new(async { Ok(Some(FieldValue::list(Vec::<FieldValue>::new()))) }),
        ));

        // Query: get resolver — returns a single item by ID.
        let get_field_name = format!("{}_get", gql_type.collection);
        let type_name = gql_type.type_name.clone();
        query = query.field(
            Field::new(
                &get_field_name,
                TypeRef::named(&type_name),
                |_ctx| FieldFuture::new(async { Ok(None::<FieldValue>) }),
            )
            .argument(async_graphql::dynamic::InputValue::new(
                "id",
                TypeRef::named_nn(TypeRef::STRING),
            )),
        );

        // Mutation: create resolver.
        let create_field_name = format!("{}_create", gql_type.collection);
        let type_name = gql_type.type_name.clone();
        mutation = mutation.field(Field::new(
            &create_field_name,
            TypeRef::named(&type_name),
            |_ctx| FieldFuture::new(async { Ok(None::<FieldValue>) }),
        ));

        // Mutation: update resolver.
        let update_field_name = format!("{}_update", gql_type.collection);
        let type_name = gql_type.type_name.clone();
        mutation = mutation.field(
            Field::new(
                &update_field_name,
                TypeRef::named(&type_name),
                |_ctx| FieldFuture::new(async { Ok(None::<FieldValue>) }),
            )
            .argument(async_graphql::dynamic::InputValue::new(
                "id",
                TypeRef::named_nn(TypeRef::STRING),
            )),
        );

        // Mutation: delete resolver.
        let delete_field_name = format!("{}_delete", gql_type.collection);
        mutation = mutation.field(
            Field::new(&delete_field_name, TypeRef::named(TypeRef::BOOLEAN), |_ctx| {
                FieldFuture::new(async { Ok(Some(FieldValue::value(false))) })
            })
            .argument(async_graphql::dynamic::InputValue::new(
                "id",
                TypeRef::named_nn(TypeRef::STRING),
            )),
        );
    }

    // async-graphql requires at least one field on root types.
    if types.is_empty() {
        query = query.field(Field::new("_empty", TypeRef::named(TypeRef::BOOLEAN), |_ctx| {
            FieldFuture::new(async { Ok(Some(FieldValue::value(true))) })
        }));
        mutation = mutation.field(Field::new("_empty", TypeRef::named(TypeRef::BOOLEAN), |_ctx| {
            FieldFuture::new(async { Ok(Some(FieldValue::value(false))) })
        }));
    }

    schema_builder
        .register(query)
        .register(mutation)
        .finish()
        .expect("dynamic schema must be valid")
}

/// Map a GraphQL scalar type name to a `TypeRef`.
fn scalar_type_ref(gql_type: &str) -> TypeRef {
    match gql_type {
        "String" => TypeRef::named(TypeRef::STRING),
        "Int" => TypeRef::named(TypeRef::INT),
        "Float" => TypeRef::named(TypeRef::FLOAT),
        "Boolean" => TypeRef::named(TypeRef::BOOLEAN),
        other => TypeRef::named(other),
    }
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
