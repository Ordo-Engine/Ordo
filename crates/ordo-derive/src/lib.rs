//! Procedural macros for Ordo rule engine
//!
//! This crate provides derive macros for generating TypedContext implementations
//! that enable zero-overhead field access in JIT-compiled code.
//!
//! # Usage
//!
//! ```ignore
//! use ordo_derive::TypedContext;
//!
//! #[derive(TypedContext)]
//! #[repr(C)]  // Recommended for stable layout
//! pub struct LoanContext {
//!     pub amount: f64,
//!     pub credit_score: i32,
//!     pub approved: bool,
//! }
//! ```
//!
//! The macro generates an implementation of `TypedContext` that provides:
//! - A static `MessageSchema` describing the struct layout
//! - Direct field pointer access via `field_ptr()`
//! - Nested field path resolution (for nested structs)

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

/// Derive macro for generating TypedContext implementations
///
/// This macro generates code that enables zero-overhead field access
/// in JIT-compiled expressions.
///
/// # Requirements
///
/// - Struct must have named fields
/// - For optimal performance, use `#[repr(C)]` to ensure stable layout
/// - Supported field types: bool, i32, i64, u32, u64, f32, f64
///
/// # Generated Code
///
/// The macro generates:
/// 1. A static `MessageSchema` with field offsets computed at compile time
/// 2. `TypedContext::schema()` returning the schema
/// 3. `TypedContext::field_ptr()` for direct field access
///
/// # Example
///
/// ```ignore
/// #[derive(TypedContext)]
/// #[repr(C)]
/// pub struct MyContext {
///     pub value: f64,      // offset: 0
///     pub count: i32,      // offset: 8
///     pub active: bool,    // offset: 12
/// }
/// ```
#[proc_macro_derive(TypedContext, attributes(typed_context))]
pub fn derive_typed_context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let name_str = name.to_string();

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "TypedContext can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "TypedContext can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    // Generate field schema entries and match arms
    let mut schema_fields = Vec::new();
    let mut match_arms = Vec::new();

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_type = &field.ty;

        // Map Rust type to FieldType
        let field_type_expr = rust_type_to_field_type(field_type);

        // Generate schema field entry
        schema_fields.push(quote! {
            ordo_core::context::FieldSchema::new(
                #field_name_str,
                #field_type_expr,
                // Use memoffset-style offset calculation
                {
                    let uninit = ::std::mem::MaybeUninit::<#name>::uninit();
                    let base_ptr = uninit.as_ptr();
                    let field_ptr = unsafe { ::std::ptr::addr_of!((*base_ptr).#field_name) };
                    (field_ptr as usize) - (base_ptr as usize)
                },
            )
        });

        // Generate match arm for field_ptr
        match_arms.push(quote! {
            #field_name_str => ::std::option::Option::Some((
                ::std::ptr::addr_of!(self.#field_name) as *const u8,
                #field_type_expr,
            ))
        });
    }

    // Generate the implementation
    let expanded = quote! {
        impl ordo_core::expr::jit::TypedContext for #name {
            fn schema() -> &'static ordo_core::context::MessageSchema {
                use ::std::sync::OnceLock;

                static SCHEMA: OnceLock<ordo_core::context::MessageSchema> = OnceLock::new();
                SCHEMA.get_or_init(|| {
                    ordo_core::context::MessageSchema::new(
                        #name_str,
                        vec![
                            #(#schema_fields,)*
                        ],
                    )
                })
            }

            unsafe fn field_ptr(
                &self,
                field_name: &str,
            ) -> ::std::option::Option<(*const u8, ordo_core::context::FieldType)> {
                match field_name {
                    #(#match_arms,)*
                    _ => ::std::option::Option::None,
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert a Rust type to the corresponding FieldType expression
fn rust_type_to_field_type(ty: &Type) -> proc_macro2::TokenStream {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    match type_str.as_str() {
        "bool" => quote!(ordo_core::context::FieldType::Bool),
        "i32" => quote!(ordo_core::context::FieldType::Int32),
        "i64" => quote!(ordo_core::context::FieldType::Int64),
        "u32" => quote!(ordo_core::context::FieldType::UInt32),
        "u64" => quote!(ordo_core::context::FieldType::UInt64),
        "f32" => quote!(ordo_core::context::FieldType::Float32),
        "f64" => quote!(ordo_core::context::FieldType::Float64),
        "String" | "::std::string::String" | "std::string::String" => {
            quote!(ordo_core::context::FieldType::String)
        }
        "Vec<u8>" | "::std::vec::Vec<u8>" => {
            quote!(ordo_core::context::FieldType::Bytes)
        }
        _ => {
            // For unknown types, try to treat as a nested message
            // This requires the nested type to also implement TypedContext
            quote! {
                ordo_core::context::FieldType::Message(
                    ::std::sync::Arc::new(<#ty as ordo_core::expr::jit::TypedContext>::schema().clone())
                )
            }
        }
    }
}

/// Derive macro for generating TypedContext for prost-generated types
///
/// This is similar to TypedContext but specifically handles prost attributes
/// to extract proto tag numbers.
#[proc_macro_derive(ProstTypedContext, attributes(prost))]
pub fn derive_prost_typed_context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let name_str = name.to_string();

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ProstTypedContext can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input,
                "ProstTypedContext can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate field schema entries and match arms
    let mut schema_fields = Vec::new();
    let mut match_arms = Vec::new();

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_type = &field.ty;

        // Extract proto tag from #[prost(..., tag = "N")] if present
        let proto_tag = extract_prost_tag(&field.attrs);

        // Map Rust type to FieldType
        let field_type_expr = rust_type_to_field_type(field_type);

        // Generate schema field entry with proto tag
        let schema_field = if let Some(tag) = proto_tag {
            quote! {
                ordo_core::context::FieldSchema::new(
                    #field_name_str,
                    #field_type_expr,
                    {
                        let uninit = ::std::mem::MaybeUninit::<#name>::uninit();
                        let base_ptr = uninit.as_ptr();
                        let field_ptr = unsafe { ::std::ptr::addr_of!((*base_ptr).#field_name) };
                        (field_ptr as usize) - (base_ptr as usize)
                    },
                ).with_proto_tag(#tag)
            }
        } else {
            quote! {
                ordo_core::context::FieldSchema::new(
                    #field_name_str,
                    #field_type_expr,
                    {
                        let uninit = ::std::mem::MaybeUninit::<#name>::uninit();
                        let base_ptr = uninit.as_ptr();
                        let field_ptr = unsafe { ::std::ptr::addr_of!((*base_ptr).#field_name) };
                        (field_ptr as usize) - (base_ptr as usize)
                    },
                )
            }
        };

        schema_fields.push(schema_field);

        // Generate match arm for field_ptr
        match_arms.push(quote! {
            #field_name_str => ::std::option::Option::Some((
                ::std::ptr::addr_of!(self.#field_name) as *const u8,
                #field_type_expr,
            ))
        });
    }

    // Generate the implementation
    let expanded = quote! {
        impl ordo_core::expr::jit::TypedContext for #name {
            fn schema() -> &'static ordo_core::context::MessageSchema {
                use ::std::sync::OnceLock;

                static SCHEMA: OnceLock<ordo_core::context::MessageSchema> = OnceLock::new();
                SCHEMA.get_or_init(|| {
                    ordo_core::context::MessageSchema::new(
                        #name_str,
                        vec![
                            #(#schema_fields,)*
                        ],
                    )
                })
            }

            unsafe fn field_ptr(
                &self,
                field_name: &str,
            ) -> ::std::option::Option<(*const u8, ordo_core::context::FieldType)> {
                match field_name {
                    #(#match_arms,)*
                    _ => ::std::option::Option::None,
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Extract the proto tag number from prost attributes
fn extract_prost_tag(attrs: &[syn::Attribute]) -> Option<u32> {
    for attr in attrs {
        if attr.path().is_ident("prost") {
            // Parse the attribute content to find tag = "N"
            if let Ok(syn::Meta::List(list)) = attr.parse_args::<syn::Meta>() {
                for nested in list.tokens.clone().into_iter() {
                    let token_str = nested.to_string();
                    if token_str.starts_with("tag") {
                        // Extract the number from tag = "N"
                        if let Some(num_str) = token_str
                            .split('=')
                            .nth(1)
                            .map(|s| s.trim().trim_matches('"').trim())
                        {
                            if let Ok(tag) = num_str.parse::<u32>() {
                                return Some(tag);
                            }
                        }
                    }
                }
            }

            // Fallback: try to parse as a simple token stream and look for tag
            let tokens = attr.meta.require_list().ok()?.tokens.to_string();
            for part in tokens.split(',') {
                let part = part.trim();
                if part.starts_with("tag") {
                    if let Some(num_str) = part
                        .split('=')
                        .nth(1)
                        .map(|s| s.trim().trim_matches('"').trim())
                    {
                        if let Ok(tag) = num_str.parse::<u32>() {
                            return Some(tag);
                        }
                    }
                }
            }
        }
    }
    None
}
