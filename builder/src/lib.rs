use itertools::Itertools;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let builder_ident = format_ident!("{}Builder", ident);
    let fields = if let Data::Struct(data_struct) = input.data {
        data_struct.fields
    } else {
        unimplemented!()
    };
    let (builder_fields, setter_fns, struct_lets, struct_fields): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) =
        fields
            .iter()
            .map(|field| {
                let ident = field.ident.as_ref().unwrap();

                let mut struct_let = quote!(
                    // For original Option field don't need ok_or
                    let #ident = self.#ident.ok_or(format!(
                        "field \"{}\" required, but not set yet.",
                        stringify!(#ident),
                    ))?;
                );

                let mut setter_ty = &field.ty;
                // For foo: std::option::Option<(String,u32)>, setter_fn as `foo((String,u32))`
                if let syn::Type::Path(type_path) = setter_ty {
                    if let Some(last_segment) = type_path.path.segments.last() {
                        if last_segment.ident.to_string() == "Option" {
                            if let syn::PathArguments::AngleBracketed(angle_args) =
                                &last_segment.arguments
                            {
                                angle_args.args.iter().for_each(|a| {
                                    if let syn::GenericArgument::Type(inner_typr) = a {
                                        setter_ty = inner_typr;
                                        struct_let = quote!(let #ident = self.#ident;);
                                    }
                                });
                            }
                        }
                    }
                }

                let builder_field = quote!(#ident: Option<#setter_ty>,);
                let setter_fn = quote!(
                        pub fn #ident(mut self, value: #setter_ty) -> Self {
                            self.#ident = Some(value);
                            self
                    }
                );
                let struct_field = quote!(#ident,);
                (builder_field, setter_fn, struct_let, struct_field)
            })
            .multiunzip();

    quote! {
        impl #ident {
            pub fn builder() -> #builder_ident {
                Default::default()
            }
        }

        #[derive(Default)]
        pub struct #builder_ident {
            #(#builder_fields)*
        }

        impl #builder_ident {
            #(#setter_fns)*

            pub fn build(self) -> Result<#ident, String> {
                #(#struct_lets)*
                Ok(#ident { #(#struct_fields)* })
            }
        }
    }
    .into()
}
