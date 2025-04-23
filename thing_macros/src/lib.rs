use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Kind)]
pub fn derive_kind(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let type_name = name.to_string();

    let uuid_seed = format!("Kind:{}", type_name);

    let expanded = quote! {
        impl things::Kind for #name {
            fn type_name(&self) -> &'static str {
                #type_name
            }

            fn uuid() -> uuid::Uuid {
                uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, #uuid_seed.as_bytes())
            }

            fn as_serialize(&self) -> &dyn erased_serde::Serialize {
                self
            }

            fn clone_box(&self) -> Box<dyn things::Kind> {
                Box::new(self.clone())
            }
        }

        erased_serde::serialize_trait_object!(things::Kind);
    };

    TokenStream::from(expanded)
}
