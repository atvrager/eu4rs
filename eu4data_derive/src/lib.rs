use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Data, DeriveInput, Fields, GenericArgument, Meta, PathArguments, Token, Type,
    parse_macro_input,
};

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

/// Checks if a field has #[serde(flatten)]
fn has_flatten_attr(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("serde") {
            // Parse comma-separated list of metas: #[serde(flatten, skip)]
            if let Ok(nested) =
                attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            {
                for meta in nested {
                    match meta {
                        Meta::Path(path) if path.is_ident("flatten") => return true,
                        _ => {}
                    }
                }
            }
        }
    }
    false
}

/// Check if a type is Option<Vec<T>>
fn is_option_vec(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(Type::Path(inner_path))) = args.args.first()
        && let Some(inner_segment) = inner_path.path.segments.last()
    {
        return inner_segment.ident == "Vec";
    }
    false
}

/// Extract T from Option<Vec<T>>
fn extract_vec_inner_type(ty: &Type) -> &Type {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(Type::Path(vec_path))) = args.args.first()
        && let Some(vec_segment) = vec_path.path.segments.last()
        && vec_segment.ident == "Vec"
        && let PathArguments::AngleBracketed(vec_args) = &vec_segment.arguments
        && let Some(GenericArgument::Type(inner)) = vec_args.args.first()
    {
        return inner;
    }
    panic!("Expected Option<Vec<T>> type")
}

/// Derive macro for generating a Deserialize implementation that tolerates duplicate keys.
///
/// This macro generates a custom Deserialize visitor that:
/// - Collects duplicate keys into Vec fields (if the field type is Option<Vec<T>>)
/// - Uses last-value-wins for non-Vec fields
/// - Skips unknown fields silently
#[proc_macro_derive(TolerantDeserialize)]
pub fn derive_tolerant_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("TolerantDeserialize only supports structs with named fields"),
        },
        _ => panic!("TolerantDeserialize only supports structs"),
    };

    // Analyze each field to determine if it's Option<Vec<T>>
    let field_info: Vec<_> = fields
        .iter()
        .map(|f| {
            let field_name = f.ident.as_ref().unwrap();
            let field_name_str = field_name.to_string();
            let field_type = &f.ty;

            // Check if type is Option<Vec<T>>
            let is_vec_field = is_option_vec(field_type);

            (field_name, field_name_str, field_type, is_vec_field)
        })
        .collect();

    // Generate visitor struct fields (Vec for accumulation)
    let visitor_field_decls: Vec<_> = field_info
        .iter()
        .map(|(name, _, ty, is_vec)| {
            if *is_vec {
                // For Option<Vec<T>>, extract T and use Vec<T> for accumulation
                let inner_type = extract_vec_inner_type(ty);
                quote! { #name: Vec<#inner_type> }
            } else {
                // For other types, use the type directly (last-wins)
                quote! { #name: #ty }
            }
        })
        .collect();

    // Generate match arms for field assignment
    let match_arms: Vec<_> = field_info
        .iter()
        .map(|(name, name_str, _, is_vec)| {
            if *is_vec {
                quote! {
                    #name_str => {
                        // Accumulate into vec
                        self.#name.push(map.next_value()?);
                    }
                }
            } else {
                quote! {
                    #name_str => {
                        // Last-wins
                        self.#name = map.next_value()?;
                    }
                }
            }
        })
        .collect();

    // Generate field names for Default impl
    let field_names: Vec<_> = field_info.iter().map(|(name, _, _, _)| name).collect();

    // Generate final struct construction
    let field_constructions: Vec<_> = field_info
        .iter()
        .map(|(name, _, _, is_vec)| {
            if *is_vec {
                quote! {
                    #name: if self.#name.is_empty() { None } else { Some(self.#name) }
                }
            } else {
                quote! {
                    #name: self.#name
                }
            }
        })
        .collect();

    let expanded = quote! {
        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct Visitor {
                    #(#visitor_field_decls),*
                }

                impl Default for Visitor {
                    fn default() -> Self {
                        Self {
                            #(#field_names: Default::default()),*
                        }
                    }
                }

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = #name;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(concat!("struct ", stringify!(#name)))
                    }

                    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        while let Some(key) = map.next_key::<String>()? {
                            match key.as_str() {
                                #(#match_arms)*
                                _ => {
                                    // Unknown field, skip it
                                    let _ = map.next_value::<serde::de::IgnoredAny>()?;
                                }
                            }
                        }

                        Ok(#name {
                            #(#field_constructions),*
                        })
                    }
                }

                deserializer.deserialize_map(Visitor::default())
            }
        }
    };

    TokenStream::from(expanded)
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

            // If it's a catch-all field, emit a special "*" marker
            if has_flatten_attr(&f.attrs) {
                return Some(quote! {
                    crate::coverage::FieldInfo {
                        name: "*",
                        visualized: false,
                        simulated: false,
                    }
                });
            }

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
