//! `#[derive(Widget)]` — generates a hidden builder for a widget struct so it
//! can be constructed with named arguments, with compile-time checking of
//! required fields.
//!
//! ```ignore
//! #[derive(Widget)]
//! struct Container {
//!     #[widget(optional)]   // may be omitted, falls back to Default::default()
//!     padding: Padding,
//!     child: Widget,        // required: forgetting it is a compile error
//! }
//! ```
//!
//! `#[widget(optional)]` on the struct makes every field optional.

mod model;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned};
use syn::parse::ParseStream;
use syn::{
    Attribute, Data, DeriveInput, Expr, Fields, Ident, Token, Type, parse_macro_input, token,
};

#[proc_macro_derive(Widget, attributes(widget))]
pub fn derive_widget(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// `#[derive(Model)]`: `to_json`, `from_json` and serde impls.
/// See `model.rs`.
#[proc_macro_derive(Model)]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct Field {
    ident: Ident,
    ty: Type,
    optional: bool,
    /// `///` comments on the field; copied to its setter so hover shows them.
    docs: Vec<Attribute>,
}

/// Parameter type and conversion expression for a field's setter.
///  - numbers/bool/char: exact type, so `8.0` infers to `f32` (not `f64`)
///  - `Vec<T>`: any iterator of `T`, so `children: [a, b]` works
///  - `Rc<dyn Trait>`: any `impl Trait` (e.g. a closure), wrapped in `Rc::new`
///  - everything else: anything that is `Into<T>`
fn setter_param(ty: &Type) -> (TokenStream2, TokenStream2) {
    if is_primitive(ty) {
        return (quote!(#ty), quote!(value));
    }
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Rc" {
                // `Rc<dyn Trait>`: `Into` can't unsize a concrete value (e.g. a
                // closure), so accept `impl Trait` and wrap it in `Rc::new`.
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(Type::TraitObject(obj))) =
                        args.args.first()
                    {
                        let bounds = &obj.bounds;
                        return (
                            quote!(impl #bounds + 'static),
                            quote!(::std::rc::Rc::new(value)),
                        );
                    }
                }
            }
            if seg.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(item)) = args.args.first() {
                        return (
                            quote!(impl ::core::iter::IntoIterator<Item = #item>),
                            quote!(::core::iter::Iterator::collect(
                                ::core::iter::IntoIterator::into_iter(value)
                            )),
                        );
                    }
                }
            }
        }
    }
    (quote!(impl ::core::convert::Into<#ty>), quote!(value.into()))
}

/// Parses `#[widget(optional)]`; returns whether it is present.
fn has_optional(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut found = false;
    for attr in attrs.iter().filter(|a| a.path().is_ident("widget")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("optional") {
                found = true;
                Ok(())
            } else {
                Err(meta.error("unknown option, expected `optional`"))
            }
        })?;
    }
    Ok(found)
}

/// Numeric/bool/char setters take the exact type, so literals like `8.0`
/// infer to `f32` instead of falling back to `f64`.
fn is_primitive(ty: &Type) -> bool {
    const PRIMS: &[&str] = &[
        "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "bool", "char",
    ];
    matches!(ty, Type::Path(p) if p.qself.is_none()
        && PRIMS.iter().any(|n| p.path.is_ident(n)))
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    // Generated items share the struct's visibility, so a `pub` widget in a
    // library can be built from another crate.
    let vis = &input.vis;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(Widget)] does not support generic structs",
        ));
    }

    let named = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "#[derive(Widget)] needs a struct with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "#[derive(Widget)] can only be used on structs",
            ));
        }
    };

    let all_optional = has_optional(&input.attrs)?;
    let mut fields = Vec::new();
    for f in named {
        fields.push(Field {
            ident: f.ident.clone().unwrap(),
            ty: f.ty.clone(),
            optional: all_optional || has_optional(&f.attrs)?,
            docs: f.attrs.iter().filter(|a| a.path().is_ident("doc")).cloned().collect(),
        });
    }

    let required: Vec<&Field> = fields.iter().filter(|f| !f.optional).collect();
    let optional: Vec<&Field> = fields.iter().filter(|f| f.optional).collect();

    let builder = format_ident!("{}Builder", name);
    let unset = format_ident!("{}Unset", name);

    // One generic parameter per required field: `Unset` until it is set.
    let gens: Vec<Ident> = (0..required.len()).map(|i| format_ident!("__T{}", i)).collect();
    // One marker trait per required field; only implemented once the field is
    // set, with a readable compiler message otherwise.
    let traits: Vec<Ident> = required
        .iter()
        .map(|f| format_ident!("{}Has_{}", name, f.ident))
        .collect();

    let req_idents: Vec<&Ident> = required.iter().map(|f| &f.ident).collect();
    let req_tys: Vec<&Type> = required.iter().map(|f| &f.ty).collect();
    let opt_idents: Vec<&Ident> = optional.iter().map(|f| &f.ident).collect();
    let opt_tys: Vec<&Type> = optional.iter().map(|f| &f.ty).collect();

    let unset_repeat: Vec<&Ident> = gens.iter().map(|_| &unset).collect();

    let trait_defs =required.iter().zip(&traits).map(|(f, tr)| {
        let field = f.ident.to_string();
        let message = format!("`{name}` is missing the required field `{field}`");
        let label = format!("set `{field}: ...`");
        quote! {
            #[diagnostic::on_unimplemented(message = #message, label = #label)]
            #vis trait #tr<T> { fn __take(self) -> T; }
            impl<T> #tr<T> for T { fn __take(self) -> T { self } }
        }
    });

    // Setters for required fields: move generic param `i` from Unset -> field type.
    let req_setters = required.iter().enumerate().map(|(i, f)| {
        let id = &f.ident;
        let ty = &f.ty;
        let out_gens = gens.iter().enumerate().map(|(j, g)| {
            if j == i { quote!(#ty) } else { quote!(#g) }
        });
        let others = required
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, o)| &o.ident);
        let (param, conv) = setter_param(ty);
        let docs = &f.docs;
        quote! {
            #(#docs)*
            #vis fn #id(self, value: #param) -> #builder<#(#out_gens),*> {
                #builder {
                    #(#others: self.#others,)*
                    #id: #conv,
                    #(#opt_idents: self.#opt_idents,)*
                }
            }
        }
    });

    let opt_setters = optional.iter().map(|f| {
        let id = &f.ident;
        let ty = &f.ty;
        let (param, conv) = setter_param(ty);
        let docs = &f.docs;
        quote! {
            #(#docs)*
            #vis fn #id(mut self, value: #param) -> Self {
                self.#id = #conv;
                self
            }
        }
    });

    Ok(quote! {
        #[allow(non_camel_case_types, non_snake_case, dead_code)]
        const _: () = {
            #vis struct #unset;

            #vis struct #builder<#(#gens),*> {
                #(#req_idents: #gens,)*
                #(#opt_idents: #opt_tys,)*
            }

            #(#trait_defs)*

            impl #name {
                #vis fn builder() -> #builder<#(#unset_repeat),*> {
                    #builder {
                        #(#req_idents: #unset,)*
                        #(#opt_idents: ::core::default::Default::default(),)*
                    }
                }
            }

            impl<#(#gens),*> #builder<#(#gens),*> {
                #(#req_setters)*
                #(#opt_setters)*
            }

            impl<#(#gens),*> #builder<#(#gens),*> {
                // The bounds live on the method (not the impl) so that rustc
                // reports a trait-bound error using our custom message.
                #vis fn build(self) -> #name
                where
                    #(#gens: #traits<#req_tys>,)*
                {
                    #name {
                        #(#req_idents: self.#req_idents.__take(),)*
                        #(#opt_idents: self.#opt_idents,)*
                    }
                }
            }
        };
    })
}

// ---------------------------------------------------------------------------
// ui!
// ---------------------------------------------------------------------------

/// ```ignore
/// ui!(Column(
///     padding: Padding(left: 8.0),
///     children: [
///         Text(text: "one"),
///         Text(text: "two"),
///     ],
/// ))
/// ```
///
/// Values:
///  - `UpperCaseName(...)`  nested widget
///  - `[a, b, ...]`         list; each item is a widget or an expression
///  - anything else         an ordinary Rust expression
///    (wrap `Some(..)`-like values in parentheses)
///
/// Generated code is spanned at the widget name, so errors point at your code.
///
/// The parser is deliberately forgiving: half-typed input (`Container(pa`,
/// a missing `:`, ...) still expands to the pieces that did parse, plus a
/// `compile_error!`. That keeps rust-analyzer completion and hover alive
/// while you type.
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
    let input: TokenStream2 = input.into();
    let mut errors = Vec::new();
    let tokens = match syn::parse::Parser::parse2(
        |stream: ParseStream| Ok(parse_call(stream, &mut errors)),
        input,
    ) {
        Ok(Some(call)) => call.expand(),
        Ok(None) => quote!(()),
        Err(e) => {
            errors.push(e);
            quote!(())
        }
    };
    if errors.is_empty() {
        return tokens.into();
    }
    let errors = errors.iter().map(syn::Error::to_compile_error);
    quote!({ #(#errors)* #tokens }).into()
}

struct WidgetCall {
    name: Ident,
    /// `None` when the user has typed only the name so far.
    fields: Option<Vec<(Ident, Option<Value>)>>,
}

enum Value {
    Widget(WidgetCall),
    List(Vec<Value>),
    Expr(Expr),
}

fn starts_widget(input: ParseStream) -> bool {
    if !(input.peek(Ident) && input.peek2(token::Paren)) {
        return false;
    }
    let fork = input.fork();
    match fork.parse::<Ident>() {
        Ok(id) => id.to_string().chars().next().is_some_and(char::is_uppercase),
        Err(_) => false,
    }
}

/// Skips to just after the next top-level comma (or the end).
fn recover(input: ParseStream) {
    while !input.is_empty() {
        if input.peek(Token![,]) {
            let _ = input.parse::<Token![,]>();
            return;
        }
        if input.parse::<proc_macro2::TokenTree>().is_err() {
            return;
        }
    }
}

fn parse_call(input: ParseStream, errors: &mut Vec<syn::Error>) -> Option<WidgetCall> {
    let name: Ident = match input.parse() {
        Ok(n) => n,
        Err(e) => {
            errors.push(e);
            return None;
        }
    };
    if !input.peek(token::Paren) {
        errors.push(syn::Error::new(name.span(), "expected `(` after the widget name"));
        return Some(WidgetCall { name, fields: None });
    }
    let group: proc_macro2::Group = match input.parse() {
        Ok(g) => g,
        Err(e) => {
            errors.push(e);
            return Some(WidgetCall { name, fields: None });
        }
    };
    let fields = parse_fields(group.stream(), errors);
    Some(WidgetCall { name, fields: Some(fields) })
}

fn parse_fields(stream: TokenStream2, errors: &mut Vec<syn::Error>) -> Vec<(Ident, Option<Value>)> {
    let mut fields = Vec::new();
    let _ = syn::parse::Parser::parse2(
        |content: ParseStream| {
            while !content.is_empty() {
                let field: Ident = match content.parse() {
                    Ok(f) => f,
                    Err(e) => {
                        errors.push(e);
                        recover(content);
                        continue;
                    }
                };
                if content.parse::<Token![:]>().is_err() {
                    // No error here: the expansion calls `.field()` without an
                    // argument, so rustc reports "takes 1 argument" at the field
                    // itself, and rust-analyzer can still complete the name.
                    fields.push((field, None));
                    recover(content);
                    continue;
                }
                let value = parse_value(content, errors);
                if value.is_none() {
                    recover(content);
                } else if !content.is_empty() {
                    if let Err(e) = content.parse::<Token![,]>() {
                        errors.push(e);
                        recover(content);
                    }
                }
                fields.push((field, value));
            }
            Ok(())
        },
        stream,
    );
    fields
}

fn parse_value(input: ParseStream, errors: &mut Vec<syn::Error>) -> Option<Value> {
    if starts_widget(input) {
        return parse_call(input, errors).map(Value::Widget);
    }
    if input.peek(token::Bracket) {
        let group: proc_macro2::Group = match input.parse() {
            Ok(g) => g,
            Err(e) => {
                errors.push(e);
                return None;
            }
        };
        let mut items = Vec::new();
        let _ = syn::parse::Parser::parse2(
            |inner: ParseStream| {
                while !inner.is_empty() {
                    match parse_value(inner, errors) {
                        Some(v) => items.push(v),
                        None => recover(inner),
                    }
                    if !inner.is_empty() {
                        if let Err(e) = inner.parse::<Token![,]>() {
                            errors.push(e);
                            recover(inner);
                        }
                    }
                }
                Ok(())
            },
            group.stream(),
        );
        return Some(Value::List(items));
    }
    match input.parse::<Expr>() {
        Ok(e) => Some(Value::Expr(e)),
        Err(e) => {
            errors.push(e);
            None
        }
    }
}

impl Value {
    fn expand(&self) -> TokenStream2 {
        match self {
            Value::Widget(w) => w.expand(),
            Value::Expr(e) => quote!(#e),
            // Each item is converted on its own, so a list may mix widget types.
            Value::List(items) => {
                let items = items.iter().map(|v| {
                    let v = v.expand();
                    quote!(::core::convert::Into::into(#v))
                });
                quote!([#(#items),*])
            }
        }
    }
}

impl WidgetCall {
    fn expand(&self) -> TokenStream2 {
        let name = &self.name;
        let span = name.span();
        let Some(fields) = &self.fields else {
            return quote!(#name);
        };
        let setters = fields.iter().map(|(field, value)| match value {
            Some(v) => {
                let v = v.expand();
                quote!(.#field(#v))
            }
            // keeps the identifier in the output so completion has a target
            None => quote!(.#field()),
        });
        // Literal tokens here take `span` (the widget name); interpolated user
        // tokens keep their own spans.
        quote_spanned!(span=> #name::builder() #(#setters)* .build())
    }
}
