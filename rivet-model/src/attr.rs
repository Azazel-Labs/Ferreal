//! The arguments of Rivet's attributes: `#[uproperty(EditAnywhere, category = "Text",
//! meta(ClampMin = 1), default = 24.0)]` and the like.

use proc_macro2::{Span, TokenStream};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{Expr, ExprLit, ExprUnary, Lit, UnOp};

/// One argument.
#[derive(Clone, Debug)]
pub struct Arg {
    /// The last segment of its path (`name` of `name = value`).
    pub name: String,
    /// Its whole path (`crate::Base` in `bases(crate::Base)`).
    pub path: syn::Path,
    pub value: ArgValue,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ArgValue {
    /// `Name`
    Flag,
    /// `name = value`
    Value(Box<Expr>),
    /// `name(...)`
    List(Vec<Arg>),
}

/// Parses the arguments of an attribute (what is inside its parentheses).
pub fn parse_args(tokens: TokenStream) -> syn::Result<Vec<Arg>> {
    let mut out = Vec::new();
    let parser = syn::meta::parser(|meta| {
        out.push(parse_meta(&meta)?);
        Ok(())
    });
    parser.parse2(tokens)?;
    Ok(out)
}

fn parse_meta(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Arg> {
    let name = meta
        .path
        .get_ident()
        .map(ToString::to_string)
        .or_else(|| meta.path.segments.last().map(|s| s.ident.to_string()))
        .ok_or_else(|| meta.error("expected a name"))?;
    let span = meta.path.span();
    let value = if meta.input.peek(syn::Token![=]) {
        ArgValue::Value(Box::new(meta.value()?.parse()?))
    } else if meta.input.peek(syn::token::Paren) {
        let mut list = Vec::new();
        meta.parse_nested_meta(|inner| {
            list.push(parse_meta(&inner)?);
            Ok(())
        })?;
        ArgValue::List(list)
    } else {
        ArgValue::Flag
    };
    Ok(Arg {
        name,
        path: meta.path.clone(),
        value,
        span,
    })
}

/// The arguments of the attributes named `name` among `attrs` (all of them, in order).
pub fn args_of(attrs: &[syn::Attribute], name: &str) -> syn::Result<Option<Vec<Arg>>> {
    let mut found: Option<Vec<Arg>> = None;
    for attr in attrs {
        if !attr.path().segments.last().is_some_and(|s| s.ident == name) {
            continue;
        }
        let args = match &attr.meta {
            syn::Meta::Path(_) => Vec::new(),
            syn::Meta::List(l) => parse_args(l.tokens.clone())?,
            syn::Meta::NameValue(nv) => {
                return Err(syn::Error::new_spanned(nv, format!("use #[{name}(...)]")));
            }
        };
        found.get_or_insert_with(Vec::new).extend(args);
    }
    Ok(found)
}

/// Whether one of `attrs` is named `name`.
pub fn has(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs
        .iter()
        .any(|a| a.path().segments.last().is_some_and(|s| s.ident == name))
}

/// The doc comment lines of `attrs`.
pub fn docs(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) => Some(s.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

impl Arg {
    pub fn error(&self, msg: impl std::fmt::Display) -> syn::Error {
        syn::Error::new(self.span, msg)
    }

    /// `name = "string"`.
    pub fn string(&self) -> syn::Result<String> {
        match &self.value {
            ArgValue::Value(e) => match &**e {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) => Ok(s.value()),
                _ => Err(self.error(format!("expected `{} = \"...\"`", self.name))),
            },
            _ => Err(self.error(format!("expected `{} = \"...\"`", self.name))),
        }
    }

    /// `name(A, B)`: the names in the list.
    pub fn names(&self) -> syn::Result<Vec<String>> {
        match &self.value {
            ArgValue::List(l) => l
                .iter()
                .map(|a| match a.value {
                    ArgValue::Flag => Ok(a.name.clone()),
                    _ => Err(a.error("expected a name")),
                })
                .collect(),
            _ => Err(self.error(format!("expected `{}(...)`", self.name))),
        }
    }

    /// `meta(Key = value, Flag)` as Unreal's `meta=(Key="value", Flag)`.
    pub fn unreal_meta(&self) -> syn::Result<String> {
        let ArgValue::List(list) = &self.value else {
            return Err(self.error("expected `meta(Key = value, ...)`"));
        };
        let items: syn::Result<Vec<String>> = list
            .iter()
            .map(|a| match &a.value {
                ArgValue::Flag => Ok(a.name.clone()),
                ArgValue::Value(e) => Ok(format!(
                    "{}=\"{}\"",
                    a.name,
                    literal_text(e).ok_or_else(|| a.error("expected a literal"))?
                )),
                ArgValue::List(_) => Err(a.error("expected `Key = value`")),
            })
            .collect();
        Ok(format!("meta=({})", items?.join(", ")))
    }
}

/// The text of a literal (a string's contents, a number or a boolean as written).
pub fn literal_text(e: &Expr) -> Option<String> {
    match e {
        Expr::Lit(ExprLit { lit, .. }) => match lit {
            Lit::Str(s) => Some(s.value()),
            Lit::Int(i) => Some(i.base10_digits().to_string()),
            Lit::Float(f) => Some(f.base10_digits().to_string()),
            Lit::Bool(b) => Some(b.value.to_string()),
            _ => None,
        },
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => literal_text(expr).map(|t| format!("-{t}")),
        Expr::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// A property's default value.
#[derive(Clone, Debug, PartialEq)]
pub enum DefaultValue {
    Bool(bool),
    /// A number as written (its digits, with a sign).
    Number(String),
    Str(String),
    /// `[1.0, 0.5, 0.2, 1.0]`: a color or vector.
    Numbers(Vec<String>),
    /// `Center` or `Align::Center`: an enum variant.
    Variant(String),
}

impl DefaultValue {
    pub fn parse(e: &Expr) -> syn::Result<DefaultValue> {
        let err =
            || syn::Error::new_spanned(e, "expected a literal, `[numbers]` or an enum variant");
        Ok(match e {
            Expr::Lit(ExprLit {
                lit: Lit::Bool(b), ..
            }) => DefaultValue::Bool(b.value),
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => DefaultValue::Str(s.value()),
            Expr::Lit(_) | Expr::Unary(_) => DefaultValue::Number(literal_text(e).ok_or_else(err)?),
            Expr::Array(a) => DefaultValue::Numbers(
                a.elems
                    .iter()
                    .map(|x| {
                        literal_text(x)
                            .filter(|t| t.parse::<f64>().is_ok())
                            .ok_or_else(err)
                    })
                    .collect::<syn::Result<_>>()?,
            ),
            Expr::Path(p) => {
                DefaultValue::Variant(p.path.segments.last().ok_or_else(err)?.ident.to_string())
            }
            _ => return Err(err()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses() {
        let args = parse_args(quote!(
            EditAnywhere,
            category = "Text",
            meta(ClampMin = 1, MultiLine = true),
            default = [1.0, -0.5]
        ))
        .unwrap();
        assert_eq!(args.len(), 4);
        assert!(matches!(args[0].value, ArgValue::Flag));
        assert_eq!(args[1].string().unwrap(), "Text");
        assert_eq!(
            args[2].unreal_meta().unwrap(),
            "meta=(ClampMin=\"1\", MultiLine=\"true\")"
        );
        let ArgValue::Value(e) = &args[3].value else {
            panic!()
        };
        assert_eq!(
            DefaultValue::parse(e).unwrap(),
            DefaultValue::Numbers(vec!["1.0".into(), "-0.5".into()])
        );
    }
}
