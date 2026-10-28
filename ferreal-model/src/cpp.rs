//! `cpp!` blocks: C++ written inline in Rust, which `ferreal-build` lifts into the module's
//! `.cpp` and the macro calls.
//!
//! ```ignore
//! let label = cpp!([actor: Obj<engine::Actor>, times: i32] -> String {
//!     return FString::Printf(TEXT("%s x%d"), *actor->GetActorNameOrLabel(), times);
//! });
//! ```
//!
//! The captures are Rust variables, typed as Ferreal passes them (see [`crate::ty`]); in the
//! C++ they are variables of the same names holding the C++ values (`AActor* actor`,
//! `int32 times`, `FString` for strings). The block is the body of a C++ function returning
//! the C++ value of the `-> Type` (nothing without one).
//!
//! Both halves find a block by its [`key`]: a hash of its tokens, so identical blocks share a
//! C++ function.

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream};

use crate::items::Param;
use crate::ty::{Ty, Use};

/// A `cpp!([captures] -> Ret { C++ })`.
#[derive(Clone, Debug)]
pub struct CppBlock {
    pub captures: Vec<Capture>,
    pub ret: Ty,
    pub body: CppBody,
}

/// The C++ of a block.
#[derive(Clone, Debug)]
pub enum CppBody {
    /// `{ ... }`: C++ that is also valid Rust tokens (`ferreal-build` copies it from the
    /// source, comments and all).
    Tokens(proc_macro2::Group),
    /// `r#"..."#`: any C++ (digit separators, `'ab'` literals...).
    Text(syn::LitStr),
}

/// A captured variable.
#[derive(Clone, Debug)]
pub struct Capture {
    pub param: Param,
    /// Its type as written.
    pub rust_ty: syn::Type,
}

impl Parse for CppBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        syn::bracketed!(content in input);
        let mut captures = Vec::new();
        while !content.is_empty() {
            let name: syn::Ident = content.parse()?;
            content.parse::<syn::Token![:]>()?;
            let ty: syn::Type = content.parse()?;
            let name = name.to_string();
            captures.push(Capture {
                param: Param {
                    // The C++ variable has the Rust variable's name.
                    cpp_name: crate::names::unraw(&name).to_string(),
                    name,
                    ty: Ty::parse(&ty, Use::Param)?,
                },
                rust_ty: ty,
            });
            if content.is_empty() {
                break;
            }
            content.parse::<syn::Token![,]>()?;
        }
        let ret = if input.peek(syn::Token![->]) {
            input.parse::<syn::Token![->]>()?;
            Ty::parse(&input.parse()?, Use::Return)?
        } else {
            Ty::Unit
        };
        let body = if input.peek(syn::LitStr) {
            CppBody::Text(input.parse()?)
        } else {
            let group: proc_macro2::Group = input.parse()?;
            if group.delimiter() != Delimiter::Brace {
                return Err(syn::Error::new(
                    group.span(),
                    "expected the C++ in braces (`cpp!([captures] -> Type { ... })`) or in a string",
                ));
            }
            CppBody::Tokens(group)
        };
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after the C++ block"));
        }
        Ok(CppBlock {
            captures,
            ret,
            body,
        })
    }
}

/// The key of the block whose macro tokens are `tokens`: a hash of their canonical text, the
/// same for the compiler's tokens (the macro) and for the parsed source's (`ferreal-build`).
pub fn key(tokens: &TokenStream) -> u64 {
    let mut text = String::new();
    canonical(tokens, &mut text);
    // FNV-1a: stable across Rust versions and platforms, unlike `DefaultHasher`.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn canonical(tokens: &TokenStream, out: &mut String) {
    let mut tokens = tokens.clone().into_iter().peekable();
    while let Some(t) = tokens.next() {
        // Doc comments (`///`, `//!` in the C++) are left out: the compiler and the parser
        // spell their `#[doc = ...]` differently.
        if let TokenTree::Punct(p) = &t
            && p.as_char() == '#'
        {
            let mut ahead = tokens.clone();
            if matches!(ahead.peek(), Some(TokenTree::Punct(b)) if b.as_char() == '!') {
                ahead.next();
            }
            if let Some(TokenTree::Group(g)) = ahead.peek()
                && g.delimiter() == Delimiter::Bracket
                && matches!(g.stream().into_iter().next(), Some(TokenTree::Ident(i)) if i == "doc")
            {
                ahead.next();
                tokens = ahead;
                continue;
            }
        }
        match t {
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);
                canonical(&g.stream(), out);
                out.push_str(close);
            }
            TokenTree::Punct(p) => {
                out.push(p.as_char());
                if p.spacing() == Spacing::Joint {
                    continue;
                }
            }
            TokenTree::Ident(i) => out.push_str(&i.to_string()),
            TokenTree::Literal(l) => out.push_str(&l.to_string()),
        }
        out.push(' ');
    }
}

/// The C++ symbol-safe name of a key.
pub fn key_name(key: u64) -> String {
    format!("{key:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses() {
        let b: CppBlock = syn::parse2(quote!([actor: Obj<engine::Actor>, times: i32] -> String {
            return actor->GetActorNameOrLabel();
        }))
        .unwrap();
        assert_eq!(b.captures.len(), 2);
        assert_eq!(b.captures[1].param.cpp_name, "times");
        assert_eq!(b.ret, Ty::String { borrowed: false });

        let b: CppBlock = syn::parse2(quote!([] { UE_LOG(LogTemp, Log, TEXT("hi")); })).unwrap();
        assert!(b.captures.is_empty());
        assert_eq!(b.ret, Ty::Unit);

        let b: CppBlock = syn::parse2(quote!([n: i32] -> i32 r#"return n * 1'000;"#)).unwrap();
        assert!(matches!(b.body, CppBody::Text(_)));

        assert!(syn::parse2::<CppBlock>(quote!([x: HashMap<u8, u8>] {})).is_err());
        assert!(syn::parse2::<CppBlock>(quote!([](x))).is_err());
    }

    #[test]
    fn keys() {
        // Spacing between tokens does not matter; tokens do.
        let a = key(&"[x: i32] -> i32 { return x*2; }".parse().unwrap());
        let b = key(&"[x : i32]->i32{return x * 2 ;}".parse().unwrap());
        let c = key(&"[x: i32] -> i32 { return x*3; }".parse().unwrap());
        assert_eq!(a, b);
        assert_ne!(a, c);
        // Joint punctuation stays joint: `->` is not `- >`.
        let d = key(&"{ a->b }".parse().unwrap());
        let e = key(&"{ a- >b }".parse().unwrap());
        assert_ne!(d, e);
        // Doc comments do not count.
        let f = key(&"{ /// A note.
 a->b }"
            .parse()
            .unwrap());
        assert_eq!(d, f);
    }
}
