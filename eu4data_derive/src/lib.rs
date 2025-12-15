//! Derive macro for automatic schema field tracking.
//!
//! Provides `#[derive(SchemaType)]` to auto-generate field metadata for coverage tracking.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, parse_macro_input};

/// Checks if a field has a specific schema attribute (e.g., `#[schema(visualized)]`)
fn has_schema_attr(attrs: &[Attribute], attr_name: &str) -> bool {
    for attr in attrs {
        if attr.path().is_ident("schema")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|nested| nested == attr_name)
        {
            return true;
        }
    }
    false
}

/// Derive macro that generates `SchemaFields` implementation for a struct.
///
/// # Usage
///
/// ```ignore
/// use eu4data_derive::SchemaType;
///
/// #[derive(SchemaType)]
/// struct ProvinceHistory {
///     #[schema(visualized)]
///     pub owner: Option<String>,
///     
///     pub base_tax: Option<f32>,  // parsed but not visualized
/// }
/// ```
#[proc_macro_derive(SchemaType, attributes(schema))]
pub fn schema_type_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract fields from struct
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "SchemaType only works on structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "SchemaType only works on structs")
                .to_compile_error()
                .into();
        }
    };

    // Generate FieldInfo for each field
    let field_infos: Vec<_> = fields
        .iter()
        .filter_map(|f| {
            let field_name = f.ident.as_ref()?;
            let name_str = field_name.to_string();
            let visualized = has_schema_attr(&f.attrs, "visualized");
            let simulated = has_schema_attr(&f.attrs, "simulated");

            Some(quote! {
                crate::coverage::FieldInfo {
                    name: #name_str,
                    visualized: #visualized,
                    simulated: #simulated,
                }
            })
        })
        .collect();

    let field_count = field_infos.len();

    let expanded = quote! {
        impl crate::coverage::SchemaFields for #name {
            fn fields() -> &'static [crate::coverage::FieldInfo] {
                static FIELDS: [crate::coverage::FieldInfo; #field_count] = [
                    #(#field_infos),*
                ];
                &FIELDS
            }
        }
    };

    TokenStream::from(expanded)
}
