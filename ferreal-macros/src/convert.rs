//! The conversions the generated Rust writes between Ferreal's types and their FFI carriers
//! (`ferreal_model::Ty::rust_ffi_in` / `rust_ffi_out`).

use ferreal_model::Ty;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// A carrier type as tokens.
pub fn ffi_type(ty: &Ty, inbound: bool) -> TokenStream {
    let s = if inbound {
        ty.rust_ffi_in()
    } else {
        ty.rust_ffi_out()
    };
    s.parse().expect("a type")
}

/// The Rust type a generated setter takes, or a property getter returns, for `ty` as given
/// in `rust_ty` (properties keep their declared type, `String` setters take `&str`).
pub fn setter_type(ty: &Ty, rust_ty: &syn::Type) -> TokenStream {
    match ty {
        Ty::String { .. } => quote!(&str),
        Ty::ObjArray { class, .. } => {
            let c = format_ident!("{}", class);
            quote!(&[::ferreal::Obj<#c>])
        }
        _ => quote!(#rust_ty),
    }
}

/// An owned Rust value from carrier value `v` (C++ to Rust). `what` names it in panics.
pub fn value_in(ty: &Ty, v: &TokenStream, what: &str) -> TokenStream {
    match ty {
        Ty::String { .. } => quote!(#v.to_string_lossy()),
        Ty::Enum(e) => {
            let e = format_ident!("{}", e);
            quote!(<#e as ::ferreal::UEnum>::from_u8(#v))
        }
        Ty::Obj { optional: true, .. } => quote!(::ferreal::Obj::from_raw(#v)),
        Ty::Obj {
            optional: false, ..
        } => quote!(::ferreal::obj_from_ffi(#v, #what)),
        // SAFETY: the glue passes a live array for the call.
        Ty::ObjArray { .. } => quote!(unsafe { ::ferreal::objs_from_ffi(#v) }),
        // SAFETY: as above.
        Ty::Slice { .. } => quote!(unsafe { #v.as_slice() }.to_vec()),
        _ => quote!(#v),
    }
}

/// Binds parameter `name` (carrier value of the same name) for a call: statements that
/// prepare it, and the argument expression (borrowing for `&str` and `&[T]`).
pub fn param_in(ty: &Ty, name: &syn::Ident, what: &str) -> (TokenStream, TokenStream) {
    let v = quote!(#name);
    match ty {
        Ty::String { borrowed: true } => (
            quote!(let #name: ::std::string::String = #name.to_string_lossy();),
            quote!(&#name),
        ),
        Ty::ObjArray { borrowed: true, .. } => {
            let conv = value_in(ty, &v, what);
            (quote!(let #name = #conv;), quote!(&#name))
        }
        // SAFETY: the glue passes a live array for the call.
        Ty::Slice { borrowed: true, .. } => (
            quote!(let #name = unsafe { #name.as_slice() };),
            quote!(#name),
        ),
        _ => {
            let conv = value_in(ty, &v, what);
            (quote!(let #name = #conv;), quote!(#name))
        }
    }
}

/// A carrier value from Rust value `v` (Rust to C++) for a return value: strings go
/// through the per-thread return buffer.
pub fn return_out(ty: &Ty, v: &TokenStream) -> TokenStream {
    match ty {
        Ty::String { .. } => quote!(::ferreal::ffi::return_str(#v)),
        Ty::Slice { .. } => quote!(::ferreal::ffi::return_slice(#v)),
        _ => arg_out_value(ty, v),
    }
}

fn arg_out_value(ty: &Ty, v: &TokenStream) -> TokenStream {
    match ty {
        Ty::Enum(_) => quote!(::ferreal::UEnum::to_u8(#v)),
        Ty::Obj { optional: true, .. } => {
            quote!(#v.map_or(::ferreal::ffi::RawObj::NULL, |o| o.raw()))
        }
        Ty::Obj {
            optional: false, ..
        } => quote!(#v.raw()),
        _ => quote!(#v),
    }
}

/// Binds argument `name` (a Rust value) as a carrier for a call into C++: statements that
/// keep what the carrier points to alive, and the carrier expression.
pub fn arg_out(ty: &Ty, name: &syn::Ident) -> (TokenStream, TokenStream) {
    match ty {
        Ty::String { .. } => (quote!(), quote!(::ferreal::ffi::Str8::new(&#name))),
        Ty::Slice { .. } => (quote!(), quote!(::ferreal::ffi::Slice::new(&#name))),
        Ty::ObjArray { .. } => {
            let raws = format_ident!("{}_raw", name);
            (
                quote!(let #raws: ::std::vec::Vec<::ferreal::ffi::RawObj> = #name.iter().map(|o| o.raw()).collect();),
                quote!(::ferreal::ffi::Slice::new(&#raws)),
            )
        }
        _ => (quote!(), arg_out_value(ty, &quote!(#name))),
    }
}
