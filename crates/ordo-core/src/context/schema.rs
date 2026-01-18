//! Context Schema System
//!
//! Provides type information for context structures, enabling Schema-Aware JIT
//! compilation with direct field access instead of runtime lookups.
//!
//! # Architecture
//!
//! ```text
//! MessageSchema
//!     ├── FieldSchema (name, type, offset)
//!     ├── FieldSchema (name, type, offset)
//!     └── FieldSchema (name, type, offset)
//!
//! SchemaRegistry
//!     ├── "LoanContext" -> MessageSchema
//!     ├── "UserProfile" -> MessageSchema
//!     └── ...
//! ```
//!
//! # Usage
//!
//! ```ignore
//! // Define schema programmatically
//! let schema = MessageSchema::builder("LoanContext")
//!     .field("amount", FieldType::Float64, 0)
//!     .field("credit_score", FieldType::Int32, 8)
//!     .build();
//!
//! // Or derive from a Protobuf message using #[derive(TypedContext)]
//! ```

use hashbrown::HashMap;
use std::sync::Arc;

/// Field type information for schema-aware compilation
#[derive(Debug, Clone)]
pub enum FieldType {
    /// Boolean value (1 byte)
    Bool,
    /// 32-bit signed integer
    Int32,
    /// 64-bit signed integer
    Int64,
    /// 32-bit unsigned integer
    UInt32,
    /// 64-bit unsigned integer
    UInt64,
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
    /// String (pointer + length + capacity on heap)
    String,
    /// Raw bytes
    Bytes,
    /// Nested message type
    Message(Arc<MessageSchema>),
    /// Repeated field (array/vector)
    Repeated(Box<FieldType>),
    /// Optional field
    Optional(Box<FieldType>),
    /// Enum (stored as i32)
    Enum(String),
}

impl FieldType {
    /// Get the size of this field type in bytes (for primitive types)
    pub fn primitive_size(&self) -> Option<usize> {
        match self {
            FieldType::Bool => Some(1),
            FieldType::Int32 | FieldType::UInt32 | FieldType::Float32 => Some(4),
            FieldType::Int64 | FieldType::UInt64 | FieldType::Float64 => Some(8),
            FieldType::Enum(_) => Some(4), // i32
            // Non-primitive types don't have a fixed size
            _ => None,
        }
    }

    /// Check if this is a numeric type that can be used in JIT expressions
    pub fn is_jit_numeric(&self) -> bool {
        matches!(
            self,
            FieldType::Bool
                | FieldType::Int32
                | FieldType::Int64
                | FieldType::UInt32
                | FieldType::UInt64
                | FieldType::Float32
                | FieldType::Float64
                | FieldType::Enum(_)
        )
    }

    /// Check if this is a primitive type (not a pointer/reference)
    pub fn is_primitive(&self) -> bool {
        self.primitive_size().is_some()
    }
}

/// Schema for a single field
#[derive(Debug, Clone)]
pub struct FieldSchema {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: FieldType,
    /// Protobuf tag number (if from protobuf)
    pub proto_tag: Option<u32>,
    /// Byte offset within the struct (calculated at compile time)
    pub offset: usize,
    /// Field size in bytes
    pub size: usize,
    /// Whether this field is required
    pub required: bool,
}

impl FieldSchema {
    /// Create a new field schema
    pub fn new(name: impl Into<String>, field_type: FieldType, offset: usize) -> Self {
        let size = field_type.primitive_size().unwrap_or(0);
        Self {
            name: name.into(),
            field_type,
            proto_tag: None,
            offset,
            size,
            required: false,
        }
    }

    /// Set the protobuf tag
    pub fn with_proto_tag(mut self, tag: u32) -> Self {
        self.proto_tag = Some(tag);
        self
    }

    /// Set the field size
    pub fn with_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    /// Mark as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

/// Schema for a message/struct type
#[derive(Debug, Clone)]
pub struct MessageSchema {
    /// Message type name
    pub name: String,
    /// All fields in this message
    pub fields: Vec<FieldSchema>,
    /// Field name to index lookup
    field_index: HashMap<String, usize>,
    /// Total struct size in bytes
    pub struct_size: usize,
}

impl MessageSchema {
    /// Create a new message schema builder
    pub fn builder(name: impl Into<String>) -> MessageSchemaBuilder {
        MessageSchemaBuilder::new(name)
    }

    /// Create a new message schema directly
    pub fn new(name: impl Into<String>, fields: Vec<FieldSchema>) -> Self {
        let field_index: HashMap<String, usize> = fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.clone(), i))
            .collect();

        let struct_size = fields.iter().map(|f| f.offset + f.size).max().unwrap_or(0);

        Self {
            name: name.into(),
            fields,
            field_index,
            struct_size,
        }
    }

    /// Get a field by name
    pub fn get_field(&self, name: &str) -> Option<&FieldSchema> {
        self.field_index.get(name).map(|&i| &self.fields[i])
    }

    /// Resolve a field path (e.g., "user.profile.age")
    pub fn resolve_field_path(&self, path: &str) -> Option<ResolvedField> {
        let parts: Vec<&str> = path.split('.').collect();
        self.resolve_field_parts(&parts)
    }

    /// Resolve field path from parts
    fn resolve_field_parts(&self, parts: &[&str]) -> Option<ResolvedField> {
        if parts.is_empty() {
            return None;
        }

        let field = self.get_field(parts[0])?;

        if parts.len() == 1 {
            return Some(ResolvedField {
                offset: field.offset,
                field_type: field.field_type.clone(),
                path: parts[0].to_string(),
            });
        }

        // Handle nested fields
        match &field.field_type {
            FieldType::Message(nested_schema) => {
                let nested = nested_schema.resolve_field_parts(&parts[1..])?;
                Some(ResolvedField {
                    offset: field.offset + nested.offset,
                    field_type: nested.field_type,
                    path: format!("{}.{}", parts[0], nested.path),
                })
            }
            _ => None, // Can't access sub-fields of non-message types
        }
    }

    /// Check if a field path exists
    pub fn has_field(&self, path: &str) -> bool {
        self.resolve_field_path(path).is_some()
    }

    /// Get all field names (including nested with dot notation)
    pub fn all_field_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        self.collect_field_paths("", &mut paths);
        paths
    }

    fn collect_field_paths(&self, prefix: &str, paths: &mut Vec<String>) {
        for field in &self.fields {
            let path = if prefix.is_empty() {
                field.name.clone()
            } else {
                format!("{}.{}", prefix, field.name)
            };

            paths.push(path.clone());

            if let FieldType::Message(nested) = &field.field_type {
                nested.collect_field_paths(&path, paths);
            }
        }
    }
}

/// A resolved field with its computed offset
#[derive(Debug, Clone)]
pub struct ResolvedField {
    /// Total byte offset from struct start
    pub offset: usize,
    /// Field type
    pub field_type: FieldType,
    /// Full path to this field
    pub path: String,
}

/// Builder for MessageSchema
pub struct MessageSchemaBuilder {
    name: String,
    fields: Vec<FieldSchema>,
    current_offset: usize,
}

impl MessageSchemaBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
            current_offset: 0,
        }
    }

    /// Add a field with automatic offset calculation
    pub fn field(mut self, name: impl Into<String>, field_type: FieldType) -> Self {
        let size = field_type.primitive_size().unwrap_or(24); // 24 for String/Vec
        let offset = self.align_offset(self.current_offset, size);

        self.fields.push(FieldSchema {
            name: name.into(),
            field_type,
            proto_tag: None,
            offset,
            size,
            required: false,
        });

        self.current_offset = offset + size;
        self
    }

    /// Add a field with explicit offset
    pub fn field_at(
        mut self,
        name: impl Into<String>,
        field_type: FieldType,
        offset: usize,
    ) -> Self {
        let size = field_type.primitive_size().unwrap_or(24);

        self.fields.push(FieldSchema {
            name: name.into(),
            field_type,
            proto_tag: None,
            offset,
            size,
            required: false,
        });

        self.current_offset = offset + size;
        self
    }

    /// Add a field with protobuf tag
    pub fn proto_field(
        mut self,
        name: impl Into<String>,
        field_type: FieldType,
        tag: u32,
        offset: usize,
    ) -> Self {
        let size = field_type.primitive_size().unwrap_or(24);

        self.fields.push(FieldSchema {
            name: name.into(),
            field_type,
            proto_tag: Some(tag),
            offset,
            size,
            required: false,
        });

        self.current_offset = offset + size;
        self
    }

    /// Build the message schema
    pub fn build(self) -> MessageSchema {
        MessageSchema::new(self.name, self.fields)
    }

    /// Align offset to the given alignment
    fn align_offset(&self, offset: usize, align: usize) -> usize {
        if align == 0 {
            return offset;
        }
        let remainder = offset % align;
        if remainder == 0 {
            offset
        } else {
            offset + align - remainder
        }
    }
}

/// Registry of all known schemas
#[derive(Debug, Default)]
pub struct SchemaRegistry {
    /// Schemas indexed by name
    schemas: HashMap<String, Arc<MessageSchema>>,
}

impl SchemaRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a schema
    pub fn register(&mut self, schema: MessageSchema) -> Arc<MessageSchema> {
        let name = schema.name.clone();
        let arc = Arc::new(schema);
        self.schemas.insert(name, Arc::clone(&arc));
        arc
    }

    /// Get a schema by name
    pub fn get(&self, name: &str) -> Option<Arc<MessageSchema>> {
        self.schemas.get(name).cloned()
    }

    /// Check if a schema exists
    pub fn contains(&self, name: &str) -> bool {
        self.schemas.contains_key(name)
    }

    /// Get all registered schema names
    pub fn names(&self) -> Vec<&str> {
        self.schemas.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of registered schemas
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_sizes() {
        assert_eq!(FieldType::Bool.primitive_size(), Some(1));
        assert_eq!(FieldType::Int32.primitive_size(), Some(4));
        assert_eq!(FieldType::Int64.primitive_size(), Some(8));
        assert_eq!(FieldType::Float64.primitive_size(), Some(8));
        assert_eq!(FieldType::String.primitive_size(), None);
    }

    #[test]
    fn test_message_schema_builder() {
        let schema = MessageSchema::builder("LoanContext")
            .field("amount", FieldType::Float64)
            .field("credit_score", FieldType::Int32)
            .field("approved", FieldType::Bool)
            .build();

        assert_eq!(schema.name, "LoanContext");
        assert_eq!(schema.fields.len(), 3);

        let amount = schema.get_field("amount").unwrap();
        assert_eq!(amount.offset, 0);
        assert!(matches!(amount.field_type, FieldType::Float64));

        let score = schema.get_field("credit_score").unwrap();
        assert_eq!(score.offset, 8); // After 8-byte f64
    }

    #[test]
    fn test_schema_with_explicit_offsets() {
        let schema = MessageSchema::builder("TestStruct")
            .field_at("a", FieldType::Float64, 0)
            .field_at("b", FieldType::Int32, 8)
            .field_at("c", FieldType::Float64, 16)
            .build();

        assert_eq!(schema.get_field("a").unwrap().offset, 0);
        assert_eq!(schema.get_field("b").unwrap().offset, 8);
        assert_eq!(schema.get_field("c").unwrap().offset, 16);
    }

    #[test]
    fn test_nested_schema() {
        let address_schema = Arc::new(
            MessageSchema::builder("Address")
                .field_at("zip_code", FieldType::Int32, 0)
                .field_at("city_id", FieldType::Int32, 4)
                .build(),
        );

        let user_schema = MessageSchema::builder("User")
            .field_at("id", FieldType::Int64, 0)
            .field_at("age", FieldType::Int32, 8)
            .field_at("address", FieldType::Message(address_schema), 16)
            .build();

        // Direct field access
        assert!(user_schema.has_field("id"));
        assert!(user_schema.has_field("age"));
        assert!(user_schema.has_field("address"));

        // Nested field access
        let resolved = user_schema.resolve_field_path("address.zip_code").unwrap();
        assert_eq!(resolved.offset, 16); // address offset + zip_code offset
        assert!(matches!(resolved.field_type, FieldType::Int32));
    }

    #[test]
    fn test_schema_registry() {
        let mut registry = SchemaRegistry::new();

        let loan_schema = MessageSchema::builder("LoanContext")
            .field("amount", FieldType::Float64)
            .build();

        registry.register(loan_schema);

        assert!(registry.contains("LoanContext"));
        assert!(!registry.contains("Unknown"));

        let schema = registry.get("LoanContext").unwrap();
        assert_eq!(schema.name, "LoanContext");
    }

    #[test]
    fn test_all_field_paths() {
        let inner = Arc::new(
            MessageSchema::builder("Inner")
                .field("x", FieldType::Int32)
                .field("y", FieldType::Int32)
                .build(),
        );

        let outer = MessageSchema::builder("Outer")
            .field("a", FieldType::Float64)
            .field("inner", FieldType::Message(inner))
            .build();

        let paths = outer.all_field_paths();
        assert!(paths.contains(&"a".to_string()));
        assert!(paths.contains(&"inner".to_string()));
        assert!(paths.contains(&"inner.x".to_string()));
        assert!(paths.contains(&"inner.y".to_string()));
    }

    #[test]
    fn test_field_type_jit_numeric() {
        assert!(FieldType::Bool.is_jit_numeric());
        assert!(FieldType::Int32.is_jit_numeric());
        assert!(FieldType::Float64.is_jit_numeric());
        assert!(!FieldType::String.is_jit_numeric());
        assert!(!FieldType::Bytes.is_jit_numeric());
    }
}
