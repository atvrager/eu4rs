use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

/// Derive macro for generating GUI window binding code.
///
/// # Usage
///
/// ```ignore
/// #[derive(GuiWindow)]
/// #[gui(window_name = "country_selection_panel")]
/// pub struct CountrySelectPanel {
///     #[gui(name = "selected_nation_label")]
///     pub nation_name: GuiText,
///
///     pub play_button: GuiButton,
/// }
/// ```
///
/// This generates a `bind()` method that looks up widgets by name in the GUI tree.
#[proc_macro_derive(GuiWindow, attributes(gui))]
pub fn derive_gui_window(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;
    let _window_name = extract_window_name(&input.attrs);

    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("GuiWindow only supports structs with named fields"),
        },
        _ => panic!("GuiWindow can only be derived for structs"),
    };

    // Generate binding code for each field
    let field_bindings = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Check for #[gui(name = "...")] or #[gui(optional)]
        let (widget_name, is_optional) = extract_field_attributes(&field.attrs, field_name);

        // Check if the field type is Option<T>
        let is_option_type = is_option_type(field_type);

        if is_optional || is_option_type {
            quote! {
                #field_name: binder.bind_optional(#widget_name)
            }
        } else {
            quote! {
                #field_name: binder.bind(#widget_name)
            }
        }
    });

    let expanded = quote! {
        impl #struct_name {
            /// Bind this window from a parsed GUI tree.
            ///
            /// # Arguments
            /// * `root` - The root GUI node (typically the window element)
            /// * `interner` - String interner for efficient name lookups
            ///
            /// # Returns
            /// A bound instance of the window with all widgets resolved.
            /// Missing widgets will be replaced with placeholders that log warnings.
            pub fn bind(
                root: &crate::gui::binder::GuiNode,
                interner: &crate::gui::interner::StringInterner,
            ) -> Self {
                let binder = crate::gui::binder::Binder::new(root, interner);

                Self {
                    #(#field_bindings),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Extract the window name from struct attributes
fn extract_window_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("gui")
            && let Ok(meta_list) = attr.meta.require_list()
        {
            // Parse nested meta like: window_name = "foo"
            for token in meta_list.tokens.clone() {
                // Simple string extraction - this could be more robust
                let token_str = token.to_string();
                if token_str.contains("window_name")
                    && let Some(start) = token_str.find('"')
                    && let Some(end) = token_str.rfind('"')
                {
                    return Some(token_str[start + 1..end].to_string());
                }
            }
        }
    }
    None
}

/// Extract field attributes: returns (widget_name, is_optional)
fn extract_field_attributes(attrs: &[syn::Attribute], field_name: &syn::Ident) -> (String, bool) {
    let mut widget_name = field_name.to_string();
    let mut is_optional = false;

    for attr in attrs {
        if attr.path().is_ident("gui")
            && let Ok(meta_list) = attr.meta.require_list()
        {
            let tokens_str = meta_list.tokens.to_string();

            // Check for "name = "widget_name""
            if tokens_str.contains("name")
                && let Some(start) = tokens_str.find('"')
                && let Some(end) = tokens_str.rfind('"')
            {
                widget_name = tokens_str[start + 1..end].to_string();
            }

            // Check for "optional"
            if tokens_str.contains("optional") {
                is_optional = true;
            }
        }
    }

    (widget_name, is_optional)
}

/// Check if a type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}
