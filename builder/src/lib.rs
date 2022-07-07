use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, Attribute, Data, DeriveInput, Error, Field, Ident, Result,
};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_builder(input).map_or_else(|e| TokenStream::from(e.to_compile_error()), |r| r)
}

fn derive_builder(input: DeriveInput) -> Result<TokenStream> {
    let fields = if let Data::Struct(data_struct) = input.data {
        data_struct.fields
    } else {
        return Err(Error::new(input.span(), "Builder macro for Struct only"));
    };

    let mut builder_fields = vec![];
    let mut setter_fns = vec![];
    let mut struct_lets = vec![];
    let mut struct_fields = vec![];
    for field in fields {
        let (builder_field, setter_fn, struct_let, struct_field) = parse_field(&field)?;
        builder_fields.push(builder_field);
        setter_fns.push(setter_fn);
        struct_lets.push(struct_let);
        struct_fields.push(struct_field);
    }

    let ident = input.ident;
    let builder_ident = format_ident!("{}Builder", ident);

    let res = quote! {
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
            
            pub fn build(self) -> ::core::result::Result<#ident, std::string::String> {
                #(#struct_lets)*
                Ok(#ident { #(#struct_fields)* })
            }
        }
    };
    Ok(res.into())
}

// return (builder_field, setter_fn, struct_let, struct_field)
fn parse_field(field: &Field) -> Result<(TokenStream2, TokenStream2, TokenStream2, TokenStream2)> {
    // dbg!(&field);
    let ident = field.ident.as_ref().unwrap();

    let mut struct_let = quote!(
        // For original Option field don't need ok_or
        let #ident = self.#ident.ok_or(format!(
            "field \"{}\" required, but not set yet.",
            stringify!(#ident),
        ))?;
    );

    let struct_ty = &field.ty;
    let mut builder_field = quote!(#ident: ::core::option::Option<#struct_ty>,);
    let mut setter_fn = quote!(
            pub fn #ident(mut self, value: #struct_ty) -> Self {
                self.#ident = Some(value);
                self
        }
    );
    // For foo: std::option::Option<(String,u32)>, setter_fn as `foo((String,u32))`
    // For Vec: if has builder attribute, setter_fn for each elem
    if let syn::Type::Path(type_path) = struct_ty {
        if let Some(syn::PathSegment {
            ident: ty_ident,
            arguments:
                syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }),
        }) = type_path.path.segments.last()
        {
            for arg in args {
                if let syn::GenericArgument::Type(inner_typr) = arg {
                    if ty_ident == "Option" {
                        builder_field = quote!(#ident: #struct_ty,);
                        struct_let = quote!(let #ident = self.#ident;);
                        setter_fn = quote!(
                                pub fn #ident(mut self, value: #inner_typr) -> Self {
                                    self.#ident = Some(value);
                                    self
                            }
                        );
                        break;
                    } else if ty_ident == "Vec" {
                        if let Some(each) = find_each_in_attrs(&field.attrs)? {
                            builder_field = quote!(#ident: #struct_ty,);
                            struct_let = quote!(let #ident = self.#ident;);
                            setter_fn = quote!(
                                    pub fn #each(mut self, value: #inner_typr) -> Self {
                                        self.#ident.push(value);
                                        self
                                }
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    let struct_field = quote!(#ident,);
    Ok((builder_field, setter_fn, struct_let, struct_field))
}

fn find_each_in_attrs(attrs: &[Attribute]) -> Result<Option<Ident>> {
    use syn::{Lit, Meta, MetaList, MetaNameValue, NestedMeta};
    for attr in attrs {
        let meta = attr.parse_meta()?;
        // dbg!(attrs);
        if let Meta::List(MetaList { path, nested, .. }) = meta {
            if path.is_ident("builder") {
                if let Some(NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                    path,
                    lit: Lit::Str(lit_str),
                    ..
                }))) = nested.first()
                {
                    if path.is_ident("each") {
                        return Ok(Some(format_ident!("{}", lit_str.value())));
                    }
                }
                return Err(Error::new(attr.span(), "expected `builder(each = \"...\")`"));
            }
        }
    }
    Ok(None)
}
