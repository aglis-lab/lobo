//! `#[derive(Model)]`: the Flutter-style data class helpers for a struct.
//!
//! For `struct Store { selected_user: Option<u32>, orders: Vec<Order> }` it generates:
//! - `to_json(&self) -> Value`           (Flutter `toJson`)
//! - `from_json(&Value) -> Result<Self>` (Flutter `fromJson`)
//! - `serde::Serialize` / `Deserialize` impls that go through the JSON above,
//!   so a model can contain other models.
//!
//! No `copy_with`: change state by assigning fields inside the state's update
//! (or by replacing the whole value), not by building copies.
//! Field types must be serializable.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
  let name = &input.ident;

  if !input.generics.params.is_empty() {
    return Err(syn::Error::new_spanned(
      &input.generics,
      "#[derive(Model)] does not support generic structs",
    ));
  }

  let named = match &input.data {
    Data::Struct(s) => match &s.fields {
      Fields::Named(f) => &f.named,
      _ => {
        return Err(syn::Error::new_spanned(
          name,
          "#[derive(Model)] needs a struct with named fields",
        ));
      }
    },
    _ => {
      return Err(syn::Error::new_spanned(name, "#[derive(Model)] only works on structs"));
    }
  };

  let idents: Vec<&Ident> = named.iter().filter_map(|f| f.ident.as_ref()).collect();
  let keys: Vec<String> = idents.iter().map(|i| i.to_string()).collect();

  Ok(quote! {
    impl #name {
      /// Flutter `toJson()`: each field becomes a key with its field name.
      pub fn to_json(&self) -> ::lobo::serde_json::Value {
        let mut map = ::lobo::serde_json::Map::new();
        #(
          map.insert(
            #keys.to_string(),
            ::lobo::serde_json::to_value(&self.#idents)
              .expect("model field must serialize"),
          );
        )*
        ::lobo::serde_json::Value::Object(map)
      }

      /// Flutter `fromJson()`. A missing key is read as `null`, so `Option` fields may be absent.
      pub fn from_json(
        value: &::lobo::serde_json::Value,
      ) -> ::lobo::serde_json::Result<Self> {
        ::core::result::Result::Ok(Self {
          #(
            #idents: ::lobo::serde_json::from_value(
              value
                .get(#keys)
                .cloned()
                .unwrap_or(::lobo::serde_json::Value::Null),
            )?,
          )*
        })
      }
    }

    impl ::lobo::serde::Serialize for #name {
      fn serialize<S: ::lobo::serde::Serializer>(
        &self,
        serializer: S,
      ) -> ::core::result::Result<S::Ok, S::Error> {
        ::lobo::serde::Serialize::serialize(&self.to_json(), serializer)
      }
    }

    impl<'de> ::lobo::serde::Deserialize<'de> for #name {
      fn deserialize<D: ::lobo::serde::Deserializer<'de>>(
        deserializer: D,
      ) -> ::core::result::Result<Self, D::Error> {
        let value: ::lobo::serde_json::Value =
          ::lobo::serde::Deserialize::deserialize(deserializer)?;
        Self::from_json(&value).map_err(<D::Error as ::lobo::serde::de::Error>::custom)
      }
    }
  })
}
