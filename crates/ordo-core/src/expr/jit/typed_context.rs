//! Typed Context for Schema-Aware JIT
//!
//! This module provides the `TypedContext` trait that enables zero-overhead
//! field access in JIT-compiled code by providing compile-time known field offsets.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     compile-time offset     ┌─────────────┐
//! │ Protobuf Struct │ ──────────────────────────► │  JIT Code   │
//! │  (e.g., Loan)   │     ldr d0, [ctx, #offset]  │  (native)   │
//! └─────────────────┘                             └─────────────┘
//!
//! No trampoline, no TLS, no HashMap lookup - just direct memory access!
//! ```
//!
//! # Usage
//!
//! Implement `TypedContext` for your protobuf message types, typically via
//! the `#[derive(TypedContext)]` macro:
//!
//! ```ignore
//! #[derive(prost::Message, TypedContext)]
//! pub struct LoanContext {
//!     #[prost(double, tag = "1")]
//!     pub amount: f64,
//!     #[prost(int32, tag = "2")]
//!     pub credit_score: i32,
//! }
//! ```

use crate::context::{FieldType, MessageSchema};
use std::sync::Arc;

/// Trait for typed contexts that support direct field access
///
/// Types implementing this trait can be used with Schema-Aware JIT compilation,
/// enabling zero-overhead field access through compile-time known offsets.
///
/// # Safety
///
/// The `field_ptr` method returns raw pointers. Implementors must ensure:
/// - The returned pointer is valid for the lifetime of `self`
/// - The offset and type information match the actual struct layout
/// - The struct uses `#[repr(C)]` or has a stable, known layout
pub trait TypedContext: Send + Sync {
    /// Get the schema for this context type
    ///
    /// This is typically a static reference computed once at startup.
    fn schema() -> &'static MessageSchema
    where
        Self: Sized;

    /// Get a raw pointer to a field by name
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid while `self` is alive.
    /// Caller must ensure proper type handling based on the returned FieldType.
    ///
    /// # Returns
    ///
    /// `Some((ptr, field_type))` if the field exists, `None` otherwise.
    unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)>;

    /// Get a raw pointer to a nested field by path
    ///
    /// # Arguments
    ///
    /// * `path` - Dot-separated field path, e.g., "user.address.city"
    ///
    /// # Safety
    ///
    /// Same safety requirements as `field_ptr`.
    unsafe fn nested_field_ptr(&self, path: &str) -> Option<(*const u8, FieldType)> {
        // Default implementation: only supports single-level fields
        // Override for types with nested message support
        if path.contains('.') {
            None
        } else {
            self.field_ptr(path)
        }
    }

    /// Read a field as f64 (for JIT numeric operations)
    ///
    /// This is a convenience method that handles type conversion.
    /// JIT code typically works with f64 for all numeric values.
    ///
    /// # Safety
    ///
    /// Must only be called with valid field names that exist in the schema.
    unsafe fn read_field_as_f64(&self, field_name: &str) -> Option<f64> {
        let (ptr, field_type) = self.field_ptr(field_name)?;

        Some(match field_type {
            FieldType::Float64 => *(ptr as *const f64),
            FieldType::Float32 => *(ptr as *const f32) as f64,
            FieldType::Int64 => *(ptr as *const i64) as f64,
            FieldType::Int32 => *(ptr as *const i32) as f64,
            FieldType::UInt64 => *(ptr as *const u64) as f64,
            FieldType::UInt32 => *(ptr as *const u32) as f64,
            FieldType::Bool => {
                if *(ptr as *const bool) {
                    1.0
                } else {
                    0.0
                }
            }
            FieldType::Enum(_) => *(ptr as *const i32) as f64,
            _ => return None, // Non-numeric types
        })
    }
}

/// Computed field access information for JIT compilation
///
/// This struct contains all information needed to generate direct
/// memory access code in the JIT compiler.
#[derive(Debug, Clone)]
pub struct FieldAccessInfo {
    /// Full path to the field (e.g., "user.balance")
    pub path: String,
    /// Byte offset from the start of the context struct
    pub offset: usize,
    /// Type of the field
    pub field_type: FieldType,
}

impl FieldAccessInfo {
    /// Create a new field access info
    pub fn new(path: impl Into<String>, offset: usize, field_type: FieldType) -> Self {
        Self {
            path: path.into(),
            offset,
            field_type,
        }
    }
}

/// Context wrapper that provides TypedContext for any schema
///
/// This is useful when you have a schema at runtime but don't have
/// a compile-time type. Note that this falls back to VM execution
/// since we can't provide true zero-overhead access.
pub struct DynamicTypedContext<'a> {
    /// Raw pointer to the context data
    data_ptr: *const u8,
    /// Schema for this context
    schema: Arc<MessageSchema>,
    /// Lifetime marker
    _marker: std::marker::PhantomData<&'a ()>,
}

impl DynamicTypedContext<'_> {
    /// Create a new dynamic typed context
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - `data_ptr` points to valid memory matching the schema
    /// - The memory remains valid for the lifetime `'a`
    pub unsafe fn new(data_ptr: *const u8, schema: Arc<MessageSchema>) -> Self {
        Self {
            data_ptr,
            schema,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the schema
    pub fn schema(&self) -> &MessageSchema {
        &self.schema
    }

    /// Read a field as f64
    ///
    /// # Safety
    ///
    /// The caller must ensure that `data_ptr` points to valid memory
    /// that matches the schema layout for the requested field.
    pub unsafe fn read_field_as_f64(&self, field_name: &str) -> Option<f64> {
        let resolved = self.schema.resolve_field_path(field_name)?;
        let ptr = self.data_ptr.add(resolved.offset);

        Some(match resolved.field_type {
            FieldType::Float64 => *(ptr as *const f64),
            FieldType::Float32 => *(ptr as *const f32) as f64,
            FieldType::Int64 => *(ptr as *const i64) as f64,
            FieldType::Int32 => *(ptr as *const i32) as f64,
            FieldType::UInt64 => *(ptr as *const u64) as f64,
            FieldType::UInt32 => *(ptr as *const u32) as f64,
            FieldType::Bool => {
                if *(ptr as *const bool) {
                    1.0
                } else {
                    0.0
                }
            }
            FieldType::Enum(_) => *(ptr as *const i32) as f64,
            _ => return None,
        })
    }
}

// Safety: The data pointer is only dereferenced with proper type checking
unsafe impl Send for DynamicTypedContext<'_> {}
unsafe impl Sync for DynamicTypedContext<'_> {}

/// Helper macro to generate field offset for a struct field
///
/// # Example
///
/// ```ignore
/// struct MyStruct {
///     a: f64,
///     b: i32,
/// }
///
/// let offset_a = field_offset!(MyStruct, a); // 0
/// let offset_b = field_offset!(MyStruct, b); // 8
/// ```
#[macro_export]
macro_rules! field_offset {
    ($struct:ty, $field:ident) => {{
        // Create a dummy instance at address 0 and get field address
        // This is safe because we're only computing the offset, not dereferencing
        let dummy = std::mem::MaybeUninit::<$struct>::uninit();
        let base = dummy.as_ptr() as usize;
        let field_ptr = std::ptr::addr_of!((*dummy.as_ptr()).$field) as usize;
        field_ptr - base
    }};
}

/// Helper macro to generate a TypedContext implementation for a simple struct
///
/// # Example
///
/// ```ignore
/// struct LoanContext {
///     amount: f64,
///     credit_score: i32,
/// }
///
/// impl_typed_context!(LoanContext, [
///     ("amount", Float64),
///     ("credit_score", Int32),
/// ]);
/// ```
#[macro_export]
macro_rules! impl_typed_context {
    ($struct_name:ty, [$(($field_name:literal, $field_ident:ident, $field_type:ident)),* $(,)?]) => {
        impl $crate::expr::jit::TypedContext for $struct_name {
            fn schema() -> &'static $crate::context::MessageSchema {
                use std::sync::OnceLock;
                use $crate::context::{MessageSchema, FieldSchema, FieldType};

                static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
                SCHEMA.get_or_init(|| {
                    MessageSchema::new(
                        stringify!($struct_name),
                        vec![
                            $(
                                FieldSchema::new(
                                    $field_name,
                                    FieldType::$field_type,
                                    $crate::field_offset!($struct_name, $field_ident),
                                ),
                            )*
                        ],
                    )
                })
            }

            unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, $crate::context::FieldType)> {
                match field_name {
                    $(
                        $field_name => Some((
                            std::ptr::addr_of!(self.$field_ident) as *const u8,
                            $crate::context::FieldType::$field_type,
                        )),
                    )*
                    _ => None,
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{FieldType, MessageSchema};
    use std::sync::OnceLock;

    // Test struct with known layout
    #[repr(C)]
    struct TestContext {
        amount: f64,
        count: i32,
        active: bool,
    }

    impl TypedContext for TestContext {
        fn schema() -> &'static MessageSchema {
            static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
            SCHEMA.get_or_init(|| {
                MessageSchema::builder("TestContext")
                    .field_at("amount", FieldType::Float64, 0)
                    .field_at("count", FieldType::Int32, 8)
                    .field_at("active", FieldType::Bool, 12)
                    .build()
            })
        }

        unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
            match field_name {
                "amount" => Some((
                    std::ptr::addr_of!(self.amount) as *const u8,
                    FieldType::Float64,
                )),
                "count" => Some((
                    std::ptr::addr_of!(self.count) as *const u8,
                    FieldType::Int32,
                )),
                "active" => Some((
                    std::ptr::addr_of!(self.active) as *const u8,
                    FieldType::Bool,
                )),
                _ => None,
            }
        }
    }

    #[test]
    fn test_typed_context_field_access() {
        let ctx = TestContext {
            amount: 1000.50,
            count: 42,
            active: true,
        };

        unsafe {
            assert_eq!(ctx.read_field_as_f64("amount"), Some(1000.50));
            assert_eq!(ctx.read_field_as_f64("count"), Some(42.0));
            assert_eq!(ctx.read_field_as_f64("active"), Some(1.0));
            assert_eq!(ctx.read_field_as_f64("nonexistent"), None);
        }
    }

    #[test]
    fn test_typed_context_schema() {
        let schema = TestContext::schema();
        assert_eq!(schema.name, "TestContext");
        assert!(schema.has_field("amount"));
        assert!(schema.has_field("count"));
        assert!(schema.has_field("active"));
        assert!(!schema.has_field("nonexistent"));
    }

    #[test]
    fn test_field_access_info() {
        let info = FieldAccessInfo::new("user.balance", 24, FieldType::Float64);
        assert_eq!(info.path, "user.balance");
        assert_eq!(info.offset, 24);
        assert!(matches!(info.field_type, FieldType::Float64));
    }

    #[test]
    fn test_dynamic_typed_context() {
        #[repr(C)]
        struct SimpleData {
            value: f64,
            flag: i32,
        }

        let data = SimpleData {
            value: 123.456,
            flag: 99,
        };

        let schema = Arc::new(
            MessageSchema::builder("SimpleData")
                .field_at("value", FieldType::Float64, 0)
                .field_at("flag", FieldType::Int32, 8)
                .build(),
        );

        unsafe {
            let ctx = DynamicTypedContext::new(&data as *const _ as *const u8, schema);
            assert_eq!(ctx.read_field_as_f64("value"), Some(123.456));
            assert_eq!(ctx.read_field_as_f64("flag"), Some(99.0));
        }
    }
}
