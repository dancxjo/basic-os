use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Thing)]
pub fn derive_thing(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let kind_str = name.to_string();

    let fields = match input.data {
        syn::Data::Struct(ref data) => match &data.fields {
            syn::Fields::Named(fields_named) => &fields_named.named,
            _ => panic!("#[derive(Thing)] only works on named structs"),
        },
        _ => panic!("#[derive(Thing)] only works on structs"),
    };

    let serialize_fields = fields.iter().map(|f| {
        let ident = &f.ident;
        quote! {
            v.extend_from_slice(&self.#ident.to_le_bytes());
        }
    });

    let deserialize_fields = fields.iter().enumerate().map(|(i, f)| {
        let ident = &f.ident;
        let ty = &f.ty;
        let offset = i * 8;
        quote! {
            let #ident = <#ty>::from_le_bytes(bytes[#offset..#offset+8].try_into().ok()?);
        }
    });

    let field_inits = fields.iter().map(|f| {
        let ident = &f.ident;
        quote! { #ident }
    });

    let field_count = fields.len();
    let total_bytes = quote! { #field_count * 8 };

    let expanded = quote! {
        impl Thingable for #name {
            fn kind() -> &'static str {
                #kind_str
            }

            fn serialize(&self) -> alloc::vec::Vec<u8> {
                let mut v = alloc::vec::Vec::new();
                #( #serialize_fields )*
                v
            }

            fn deserialize(bytes: &[u8]) -> Option<Self> {
                if bytes.len() != #total_bytes { return None; }
                #( #deserialize_fields )*
                Some(Self { #( #field_inits ),* })
            }
        }
    };

    TokenStream::from(expanded)
}
