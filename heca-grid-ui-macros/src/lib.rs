//! Derives the declarative property surface **from the widgets' own builder methods**.
//!
//! The defect this exists to remove: `realize` (app-side) used to hand-write a reader for every
//! property of every widget. Nothing forced that list to keep up with the library, so capabilities
//! that widgets genuinely had — an `Input`'s placeholder, a `ScrollRegion`'s second axis — were
//! simply unreachable from a description. Silently: no warning, no log, no failing test.
//!
//! So the property list is no longer written anywhere. A builder marked [`macro@prop`] *is* the
//! declarative surface, and [`macro@props`] generates the translation from it.
//!
//! ```ignore
//! #[props]
//! impl Input {
//!     #[prop] pub fn placeholder(mut self, s: impl Into<String>) -> Self { .. }
//!     #[prop] pub fn value(mut self, s: impl Into<String>) -> Self { .. }
//!     pub fn on_change(self, f: impl Fn(Action)) -> Self { .. }   // unmarked ⇒ host-only
//! }
//! ```
//!
//! **`heca-grid-ui` never learns what a plugin is.** The generated code speaks only `PropInput`,
//! a neutral scalar owned by the library; the app converts its own model into that at the boundary.
//! The crate graph (`heca` → `heca-grid-ui`, never the reverse) is untouched.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, ImplItem, ItemImpl, Type};

/// Generate `SetProp` for an `impl` block from the builders inside it marked `#[prop]`.
///
/// Emits `set_prop(self, key, value) -> Self` — **by value**, because that is the shape every
/// builder in this library already has (`fn gap(mut self, v: f32) -> Self`). A `&mut self` setter
/// could not call them without `Default` on every widget.
///
/// Also emits `PROP_NAMES` (what this widget exposes) and `HOST_ONLY_BUILDERS` (what it
/// deliberately does not), which is what the drift guard compares against the real method list.
///
/// **Properties are order-independent, and nobody has to think about that.** They are applied
/// after children are attached, so a builder that clamps against its children (`Select::selected`,
/// `Tabs::selected`) sees the real ones. There is no early/late marker and no sequencing for an
/// author, a caller or an agent to reason about. If a widget ever *does* have two properties whose
/// order matters, that is a bug in the widget — fix it there, do not push the ordering onto
/// everyone else.
#[proc_macro_attribute]
pub fn props(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemImpl);
    let self_ty = &input.self_ty;

    let mut applies = Vec::new();
    let mut exposed = Vec::new();
    let mut host_only = Vec::new();

    for item in &mut input.items {
        let ImplItem::Fn(f) = item else { continue };
        let name = f.sig.ident.clone();
        // Match the LAST path segment, so `#[prop]` and `#[heca_grid_ui_macros::prop]` both count.
        // Matching only the bare ident silently demotes every qualified marker to host-only.
        let marker = f.attrs.iter().position(|a| {
            a.path()
                .segments
                .last()
                .is_some_and(|seg| seg.ident == "prop")
        });
        let Some(idx) = marker else {
            if !takes_self_by_value(f) {
                continue; // an accessor, not a builder
            }
            // A builder must be CLASSIFIED, never classified by omission. Silence used to mean
            // "host-only", which is how a capability goes missing: nobody decides, so nobody
            // notices. Requiring the decision is the whole point of this phase, one level down.
            let Some(idx) = f.attrs.iter().position(|a| {
                a.path().segments.last().is_some_and(|s| s.ident == "host_only")
            }) else {
                return syn::Error::new_spanned(
                    &f.sig,
                    "this builder is neither `#[prop]` nor `#[host_only]`.\n\
                     Every builder must say which it is, because being left out silently is \
                     exactly how a widget capability becomes unreachable from a description.\n\
                     · `#[prop]` — a description may set it (scalars and enums-by-name).\n\
                     · `#[host_only(\"reason\")]` — it cannot come from static data: a closure \
                     (behaviour crosses as an Intent), a child (use `children`), or a live signal.",
                )
                .to_compile_error()
                .into();
            };
            f.attrs.remove(idx);
            host_only.push(name.to_string());
            continue;
        };
        let attr = f.attrs.remove(idx);
        // `#[prop]` takes NO arguments, and that is enforced rather than merely documented.
        // Any argument here would be a sequencing hint — `late`, `after = "x"` — and sequencing is
        // exactly what nobody should have to think about. Rejecting it at compile time stops the
        // concept being reintroduced one widget at a time.
        if !matches!(attr.meta, syn::Meta::Path(_)) {
            return syn::Error::new_spanned(
                attr,
                "`#[prop]` takes no arguments: properties are order-independent by construction. \
                 They are applied after children are attached, so a builder that clamps against \
                 its children already sees them. If two properties on a widget genuinely depend \
                 on each other, fix that widget's setters — do not add sequencing here.",
            )
            .to_compile_error()
            .into();
        }
        let key = name.to_string();
        let Some(arg) = f.sig.inputs.iter().nth(1) else { continue };
        let conversion = match arg_conversion(arg) {
            Some(c) => c,
            None => continue,
        };
        let arm = quote! {
            #key => match #conversion {
                Some(v) => self.#name(v),
                None => self,
            },
        };
        exposed.push(key);
        applies.push(arm);
    }

    let expanded = quote! {
        #input

        impl ::heca_grid_ui::SetProp for #self_ty {
            const PROP_NAMES: &'static [&'static str] = &[#(#exposed),*];
            const HOST_ONLY_BUILDERS: &'static [&'static str] = &[#(#host_only),*];

            fn set_prop(self, key: &str, value: &::heca_grid_ui::PropInput) -> Self {
                match key { #(#applies)* _ => self }
            }
        }
    };
    expanded.into()
}

/// True for `fn f(self, ..)` / `fn f(mut self, ..)` — i.e. the builder shape. Methods taking
/// `&self`/`&mut self` are accessors, not builders, and are not part of the property surface.
fn takes_self_by_value(f: &syn::ImplItemFn) -> bool {
    matches!(f.sig.inputs.first(), Some(syn::FnArg::Receiver(r)) if r.reference.is_none())
}

/// How a [`PropInput`] becomes this builder's argument type. Returns `None` for argument types
/// that cannot come from static data (closures, boxed components) — those stay host-only even if
/// someone marks them, rather than generating code that would not compile.
fn arg_conversion(arg: &syn::FnArg) -> Option<proc_macro2::TokenStream> {
    let syn::FnArg::Typed(pat) = arg else { return None };
    let ty: &Type = &pat.ty;
    let text = quote!(#ty).to_string().replace(' ', "");

    Some(match text.as_str() {
        "bool" => quote!(value.as_bool()),
        "f32" => quote!(value.as_f32()),
        "f64" => quote!(value.as_f32().map(|v| v as f64)),
        "usize" => quote!(value.as_f32().map(|v| v.max(0.0) as usize)),
        "u16" => quote!(value.as_f32().map(|v| v.max(0.0) as u16)),
        "String" => quote!(value.as_text().map(::std::string::ToString::to_string)),
        t if t.contains("Into<String>") => {
            quote!(value.as_text().map(::std::string::ToString::to_string))
        }
        t if t.starts_with('&') || t.contains("dyn") || t.contains("implFn") => return None,
        // Anything else is an enum carried by NAME, the same way glyphs and colours already
        // travel. `PropName` supplies the lookup, so the enum's variants are the vocabulary and
        // nobody maintains a parallel list of strings.
        _ => {
            quote!(value.as_text().and_then(<#ty as ::heca_grid_ui::PropName>::from_prop_name))
        }
    })
}

/// Derive `from_prop_name` for a fieldless enum: `SpaceBetween` ⇒ `"space_between"`.
///
/// This is what keeps enum-valued properties from becoming the next hand-maintained list — add a
/// variant and its name is accepted, with no string table to update.
#[proc_macro_derive(PropName)]
pub fn derive_prop_name(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return quote! { compile_error!("PropName only applies to enums"); }.into();
    };

    let arms = data.variants.iter().filter_map(|v| {
        if !matches!(v.fields, Fields::Unit) {
            return None;
        }
        let ident = &v.ident;
        let snake = to_snake_case(&ident.to_string());
        Some(quote!(#snake => Some(Self::#ident),))
    });
    let names = data.variants.iter().filter(|v| matches!(v.fields, Fields::Unit)).map(|v| {
        let s = to_snake_case(&v.ident.to_string());
        quote!(#s)
    });

    quote! {
        impl ::heca_grid_ui::PropName for #name {
            const VARIANT_NAMES: &'static [&'static str] = &[#(#names),*];
            fn from_prop_name(name: &str) -> Option<Self> {
                match name { #(#arms)* _ => None }
            }
        }
    }
    .into()
}

fn to_snake_case(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (i, ch) in camel.char_indices() {
        if ch.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Marks a builder as part of the declarative surface. Consumed by [`macro@props`]; on its own it
/// expands to nothing, so a marked builder is an ordinary method to every other caller.
#[proc_macro_attribute]
pub fn prop(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a builder as deliberately **not** reachable from a description, with the reason.
///
/// The reason is the point. "Unmarked" is not a decision; this is. Legitimate cases are a closure
/// (behaviour crosses as an `Intent`), composed content (a description uses `children`), and a
/// builder bound to a live host signal.
#[proc_macro_attribute]
pub fn host_only(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
