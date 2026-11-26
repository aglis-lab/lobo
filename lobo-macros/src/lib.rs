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
use syn::ext::IdentExt;
use syn::parse::ParseStream;
use syn::{
  Attribute, Block, Data, DeriveInput, Expr, Fields, Ident, Path, Stmt, Token, Type,
  parse_macro_input, token,
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
///  - `Rc<dyn Trait>` / `Box<dyn Trait>`: any `impl Trait` (e.g. a closure), wrapped in `Rc::new` or `Box::new`
///  - `Option<T>`: accepts `impl Into<T>` (wrapped in `Some`) or an `Option<T>` as is;
///    numbers/bool/char take exactly `T`
///  - everything else: anything that is `Into<T>`
fn setter_param(ty: &Type) -> (TokenStream2, TokenStream2, TokenStream2) {
  let (param, conv) = setter_param_plain(ty);
  // `Option<T>` (non-primitive) takes a `T`-convertible value or an `Option<T>`;
  // the marker type parameter `__M` tells the two conversions apart.
  if param.to_string().contains("IntoOptionArg") {
    return (quote!(<__M>), param, conv);
  }
  (quote!(), param, conv)
}

fn setter_param_plain(ty: &Type) -> (TokenStream2, TokenStream2) {
  if is_primitive(ty) {
    return (quote!(#ty), quote!(value));
  }
  if let Type::Path(p) = ty {
    if let Some(seg) = p.path.segments.last() {
      if seg.ident == "Rc" || seg.ident == "Box" {
        // `Rc<dyn Trait>` / `Box<dyn Trait>`: `Into` can't unsize a concrete value (e.g. a
        // closure), so accept `impl Trait` and wrap it in `Rc::new` or `Box::new`.
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
          if let Some(syn::GenericArgument::Type(Type::TraitObject(obj))) = args.args.first() {
            let bounds = &obj.bounds;
            let wrap = if seg.ident == "Rc" {
              quote!(::std::rc::Rc::new(value))
            } else {
              quote!(::std::boxed::Box::new(value))
            };
            return (quote!(impl #bounds + 'static), wrap);
          }
        }
      }
      if seg.ident == "Option" {
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
          if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
            // `Option<f32>`: exact type, so `width: 100.0` infers to `f32`.
            if is_primitive(inner) {
              return (quote!(#inner), quote!(::core::option::Option::Some(value)));
            }
            return (
              quote!(impl ::lobo::IntoOptionArg<#inner, __M>),
              quote!(::lobo::IntoOptionArg::into_option_arg(value)),
            );
          }
        }
      }
      if seg.ident == "Vec" {
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
          if let Some(syn::GenericArgument::Type(item)) = args.args.first() {
            return (
              quote!(impl ::core::iter::IntoIterator<Item = #item>),
              quote!(::core::iter::Iterator::collect(::core::iter::IntoIterator::into_iter(value))),
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
    "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128",
    "usize", "bool", "char",
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
      return Err(syn::Error::new_spanned(name, "#[derive(Widget)] can only be used on structs"));
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
  let traits: Vec<Ident> =
    required.iter().map(|f| format_ident!("{}Has_{}", name, f.ident)).collect();

  let req_idents: Vec<&Ident> = required.iter().map(|f| &f.ident).collect();
  let req_tys: Vec<&Type> = required.iter().map(|f| &f.ty).collect();
  let opt_idents: Vec<&Ident> = optional.iter().map(|f| &f.ident).collect();
  let opt_tys: Vec<&Type> = optional.iter().map(|f| &f.ty).collect();

  let unset_repeat: Vec<&Ident> = gens.iter().map(|_| &unset).collect();

  let trait_defs = required.iter().zip(&traits).map(|(f, tr)| {
    let field = f.ident.to_string();
    let message = format!("`{name}` is missing the required field `{field}`");
    let label = format!("set `{field}: ...`");
    quote! {
      #[diagnostic::on_unimplemented(message = #message, label = #label)]
      #vis trait #tr<T> { fn __take(self) -> T; }
      impl<T> #tr<T> for T { fn __take(self) -> T { self } }
    }
  });

  // Every field gets two setters: `field(value)`, which converts like
  // `setter_param` says, and a hidden `__lobo_set_field(value)` taking the
  // exact field type. `ui!` uses the latter after an `if`/`match`, whose
  // branches it has already converted with `Widget::__lobo_into_field`.
  // Required setters move generic param `i` from Unset -> field type.
  let req_setters = required.iter().enumerate().map(|(i, f)| {
    let id = &f.ident;
    let set = format_ident!("__lobo_set_{}", id.unraw());
    let ty = &f.ty;
    let out_gens: Vec<TokenStream2> =
      gens.iter().enumerate().map(|(j, g)| if j == i { quote!(#ty) } else { quote!(#g) }).collect();
    let others = required.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, o)| &o.ident);
    let (generics, param, conv) = setter_param(ty);
    let docs = &f.docs;
    quote! {
      #(#docs)*
      #vis fn #id #generics (self, value: #param) -> #builder<#(#out_gens),*> {
        self.#set(#conv)
      }

      #[doc(hidden)]
      #vis fn #set(self, value: #ty) -> #builder<#(#out_gens),*> {
        #builder {
          #(#others: self.#others,)*
          #id: value,
          #(#opt_idents: self.#opt_idents,)*
        }
      }
    }
  });

  let opt_setters = optional.iter().map(|f| {
    let id = &f.ident;
    let set = format_ident!("__lobo_set_{}", id.unraw());
    let ty = &f.ty;
    let (generics, param, conv) = setter_param(ty);
    let docs = &f.docs;
    quote! {
      #(#docs)*
      #vis fn #id #generics (self, value: #param) -> Self {
        self.#set(#conv)
      }

      #[doc(hidden)]
      #vis fn #set(mut self, value: #ty) -> Self {
        self.#id = value;
        self
      }
    }
  });

  // Same conversion as the setter, without a builder: used by `ui!` on each
  // branch of an `if`/`match` field value, so branches can differ in type.
  let converters = fields.iter().map(|f| {
    let into = format_ident!("__lobo_into_{}", f.ident.unraw());
    let ty = &f.ty;
    let (generics, param, conv) = setter_param(ty);
    quote! {
      #[doc(hidden)]
      #vis fn #into #generics (value: #param) -> #ty { #conv }
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
        #(#converters)*

        #[doc(hidden)]
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
/// ui!(Container {
///     padding: Padding { left: 8.0 },
///     child: Column {
///         children: [
///             Text { text: "one" },
///             if done { Text { text: "done" } } else { Spinner {} },
///         ],
///     },
/// })
/// ```
///
/// Values:
///  - `UpperCaseName { ... }`  nested widget; omitted optional fields use
///    their defaults, a missing required field is a compile error
///  - `[a, b, ...]`            list; each item is a widget or an expression
///  - `if` / `match` / `{ }`   their result expressions are values too, so
///    branches may build different widgets
///  - anything else            an ordinary Rust expression; to write a plain
///    struct literal, wrap it in parentheses: `(Point { x, y })`
///
/// Closures and function arguments are plain Rust: inside them, use `ui!`
/// again.
///
/// The whole expression evaluates to a `Widget`, so it can be returned from
/// `build` as `Some(ui!(...))` without `.into()`. This means the outermost
/// value must be an `Element` (a plain-data builder like `Padding` only works
/// nested, as a field value).
///
/// Generated code is spanned at the widget name, so errors point at your code.
///
/// The parser is deliberately forgiving: half-typed input (`Container { pa`,
/// a missing `:`, ...) still expands to the pieces that did parse, plus a
/// `compile_error!`. That keeps rust-analyzer completion and hover alive
/// while you type.
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
  let input: TokenStream2 = input.into();
  let mut errors = Vec::new();
  let tokens = match syn::parse::Parser::parse2(
    |stream: ParseStream| Ok(parse_root(stream, &mut errors)),
    input,
  ) {
    Ok(tokens) => tokens,
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

/// Only the outermost value is wrapped in `Widget::from`: nested widgets are
/// converted by their setter (`impl Into<Widget>`) or list item.
fn parse_root(input: ParseStream, errors: &mut Vec<syn::Error>) -> TokenStream2 {
  if input.is_empty() {
    errors.push(input.error("expected a widget, like `Text { text: \"hi\" }`"));
    return quote!(());
  }
  let value = parse_value(input, errors);
  if !input.is_empty() {
    errors.push(input.error("unexpected tokens after the widget"));
  }
  match value {
    Some(Value::Widget(call)) => {
      let span = call.span();
      let inner = call.expand();
      quote_spanned!(span=> ::lobo::Widget::from(#inner))
    }
    Some(Value::Flow(expr)) => wrap_tails(expr, |tail| quote!(::lobo::Widget::from(#tail))),
    Some(other) => {
      // Still emitted, so a half-typed name keeps completion working.
      let tokens = other.expand();
      errors
        .push(syn::Error::new_spanned(&tokens, "expected a widget, like `Text { text: \"hi\" }`"));
      tokens
    }
    None => quote!(()),
  }
}

struct WidgetCall {
  path: Path,
  fields: Vec<(Ident, Option<Value>)>,
}

enum Value {
  Widget(WidgetCall),
  List(Vec<Value>),
  /// `if` / `match` / block. Its result expressions are already expanded
  /// (as `Expr::Verbatim`); the parent wraps each one in a conversion.
  Flow(Expr),
  Expr(Expr),
}

fn is_widget_path(path: &Path) -> bool {
  path
    .segments
    .last()
    .is_some_and(|s| s.ident.to_string().chars().next().is_some_and(char::is_uppercase))
}

fn starts_widget(input: ParseStream) -> bool {
  let fork = input.fork();
  match fork.call(Path::parse_mod_style) {
    Ok(path) => fork.peek(token::Brace) && is_widget_path(&path),
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

/// Only called when `starts_widget` is true.
fn parse_call(input: ParseStream, errors: &mut Vec<syn::Error>) -> Option<WidgetCall> {
  let path = match input.call(Path::parse_mod_style) {
    Ok(p) => p,
    Err(e) => {
      errors.push(e);
      return None;
    }
  };
  let group: proc_macro2::Group = match input.parse() {
    Ok(g) => g,
    Err(e) => {
      errors.push(e);
      return Some(WidgetCall { path, fields: Vec::new() });
    }
  };
  let fields = parse_fields(group.stream(), errors);
  Some(WidgetCall { path, fields })
}

fn parse_fields(stream: TokenStream2, errors: &mut Vec<syn::Error>) -> Vec<(Ident, Option<Value>)> {
  let mut fields = Vec::new();
  let _ = syn::parse::Parser::parse2(
    |content: ParseStream| {
      while !content.is_empty() {
        if content.peek(Token![..]) {
          errors.push(
            content
              .error("`..` is not needed in `ui!`: omitted optional fields use their defaults"),
          );
          while !content.is_empty() {
            let _ = content.parse::<proc_macro2::TokenTree>();
          }
          break;
        }
        let field: Ident = match content.parse() {
          Ok(f) => f,
          Err(e) => {
            errors.push(e);
            recover(content);
            continue;
          }
        };
        // Shorthand `Text { text }`, as in a struct literal.
        if content.is_empty() || content.peek(Token![,]) {
          let _ = content.parse::<Token![,]>();
          let value: Expr = syn::parse_quote!(#field);
          fields.push((field, Some(Value::Expr(value))));
          continue;
        }
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
    Ok(e) => Some(Value::from_expr(e, errors)),
    Err(e) => {
      errors.push(e);
      None
    }
  }
}

/// `return`, `panic!()`, ... : left alone, since `Into::into(return)` can't
/// be inferred.
fn diverges(expr: &Expr) -> bool {
  match expr {
    Expr::Return(_) | Expr::Break(_) | Expr::Continue(_) => true,
    Expr::Macro(m) => m.mac.path.get_ident().is_some_and(|i| {
      matches!(i.to_string().as_str(), "panic" | "todo" | "unreachable" | "unimplemented")
    }),
    _ => false,
  }
}

/// Calls `f` on every result expression of an `if` / `match` / block,
/// recursing through nested ones.
fn for_each_tail(expr: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
  match expr {
    Expr::If(e) => {
      for_each_block_tail(&mut e.then_branch, f);
      if let Some((_, other)) = &mut e.else_branch {
        for_each_tail(other, f);
      }
    }
    Expr::Match(e) => {
      for arm in &mut e.arms {
        for_each_tail(&mut arm.body, f);
      }
    }
    Expr::Block(e) if e.label.is_none() => for_each_block_tail(&mut e.block, f),
    _ if diverges(expr) => {}
    _ => f(expr),
  }
}

fn for_each_block_tail(block: &mut Block, f: &mut dyn FnMut(&mut Expr)) {
  let Some(last) = block.stmts.last_mut() else { return };
  // `{ ui!(...) }` parses as a macro statement; treat it as an expression.
  if let Stmt::Macro(m) = last {
    if m.semi_token.is_none() {
      let mac = Expr::Macro(syn::ExprMacro { attrs: m.attrs.clone(), mac: m.mac.clone() });
      *last = Stmt::Expr(mac, None);
    }
  }
  if let Stmt::Expr(tail, None) = last {
    for_each_tail(tail, f);
  }
}

/// Wraps each (already expanded) result expression of a `Flow` with `wrap`.
fn wrap_tails(mut expr: Expr, wrap: impl Fn(&Expr) -> TokenStream2) -> TokenStream2 {
  for_each_tail(&mut expr, &mut |tail| *tail = Expr::Verbatim(wrap(tail)));
  quote!(#expr)
}

impl Value {
  /// For expressions parsed by `syn`: inside `if` / `match` branches and
  /// their lists.
  fn from_expr(expr: Expr, errors: &mut Vec<syn::Error>) -> Value {
    match expr {
      Expr::Struct(s) if s.qself.is_none() && is_widget_path(&s.path) => {
        if let Some(rest) = &s.rest {
          errors.push(syn::Error::new_spanned(
            rest,
            "`..` is not needed in `ui!`: omitted optional fields use their defaults",
          ));
        }
        let mut fields = Vec::new();
        for fv in s.fields {
          match fv.member {
            syn::Member::Named(id) => fields.push((id, Some(Value::from_expr(fv.expr, errors)))),
            syn::Member::Unnamed(i) => {
              errors.push(syn::Error::new_spanned(i, "`ui!` widgets need named fields"))
            }
          }
        }
        Value::Widget(WidgetCall { path: s.path, fields })
      }
      Expr::Array(a) => {
        Value::List(a.elems.into_iter().map(|e| Value::from_expr(e, errors)).collect())
      }
      mut flow @ (Expr::If(_) | Expr::Match(_) | Expr::Block(_)) => {
        for_each_tail(&mut flow, &mut |tail| {
          let taken = std::mem::replace(tail, Expr::Verbatim(TokenStream2::new()));
          *tail = Expr::Verbatim(Value::from_expr(taken, errors).expand());
        });
        Value::Flow(flow)
      }
      other => Value::Expr(other),
    }
  }

  fn expand(&self) -> TokenStream2 {
    match self {
      Value::Widget(w) => w.expand(),
      Value::Expr(e) | Value::Flow(e) => quote!(#e),
      // Each item is converted on its own, so a list may mix widget types.
      Value::List(items) => {
        let items = items.iter().map(|v| match v {
          Value::Flow(e) => wrap_tails(e.clone(), |t| quote!(::core::convert::Into::into(#t))),
          _ => {
            let v = v.expand();
            quote!(::core::convert::Into::into(#v))
          }
        });
        quote!([#(#items),*])
      }
    }
  }
}

impl WidgetCall {
  fn span(&self) -> proc_macro2::Span {
    self.path.segments.last().map_or_else(proc_macro2::Span::call_site, |s| s.ident.span())
  }

  fn expand(&self) -> TokenStream2 {
    let path = &self.path;
    let span = self.span();
    let setters = self.fields.iter().map(|(field, value)| match value {
      // Branches may differ in type (`Text` vs `Column`), so each one is
      // converted to the field's type first, then set as-is.
      Some(Value::Flow(e)) => {
        let name = field.unraw();
        let into = Ident::new(&format!("__lobo_into_{name}"), field.span());
        let set = Ident::new(&format!("__lobo_set_{name}"), field.span());
        let e = wrap_tails(e.clone(), |t| quote_spanned!(span=> #path::#into(#t)));
        quote!(.#set(#e))
      }
      Some(v) => {
        let v = v.expand();
        quote!(.#field(#v))
      }
      // keeps the identifier in the output so completion has a target
      None => quote!(.#field()),
    });
    // Literal tokens here take `span` (the widget name); interpolated user
    // tokens keep their own spans.
    quote_spanned!(span=> #path::builder() #(#setters)* .build())
  }
}
