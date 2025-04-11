use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Thing)]
pub fn derive_thing(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let kind_str = name.to_string();

    let expanded = quote! {
        impl Thingable for #name {
            fn kind() -> &'static str {
                #kind_str
            }

            fn serialize(&self) -> alloc::vec::Vec<u8> {
                postcard::to_allocvec(self).expect("Serialization failed")
            }

            fn deserialize(bytes: &[u8]) -> Option<Self> {
                postcard::from_bytes(bytes).ok()
            }
        }
    };

    TokenStream::from(expanded)
}
