//! The types that cross between Rust and Unreal, recognized from Rust syntax.
//!
//! | Rust | Unreal |
//! |---|---|
//! | `bool`, `i32`, `i64`, `u8`, `u32`, `f32`, `f64` | `bool`, `int32`, `int64`, `uint8`, `uint32`, `float`, `double` |
//! | `String`, `&str` | `FString` |
//! | `LinearColor`, `Vector2`, `Vector3` | `FLinearColor`, `FVector2D`, `FVector` |
//! | an enum declared with `#[uenum]` | its `UENUM` (`enum class E...: uint8`) |
//! | `Obj<C>`, `Option<Obj<C>>` | `C*` / `TObjectPtr<C>` (an `engine_class!` or `#[uclass]` `C`) |
//! | `Vec<Obj<C>>`, `&[Obj<C>]` | `TArray<TObjectPtr<C>>` |
//! | `Vec<T>`, `&[T]` for `T` in `f32`, `u8`, `u32`, `i32` | `TArray<T>`, `TConstArrayView<T>` (parameters; `Vec<T>` also returned) |
//!
//! Types are recognized by the last segment of their path, so `ferreal::LinearColor` and
//! `LinearColor` are the same. Any other single identifier is taken to be a `#[uenum]`
//! (the compiler reports it if it is not one).

use syn::{GenericArgument, PathArguments, Type};

/// Element types of slices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    F32,
    U8,
    U32,
    I32,
}

impl Prim {
    pub fn rust(self) -> &'static str {
        match self {
            Prim::F32 => "f32",
            Prim::U8 => "u8",
            Prim::U32 => "u32",
            Prim::I32 => "i32",
        }
    }

    pub fn cpp(self) -> &'static str {
        match self {
            Prim::F32 => "float",
            Prim::U8 => "uint8",
            Prim::U32 => "uint32",
            Prim::I32 => "int32",
        }
    }

    fn from_name(name: &str) -> Option<Prim> {
        Some(match name {
            "f32" => Prim::F32,
            "u8" => Prim::U8,
            "u32" => Prim::U32,
            "i32" => Prim::I32,
            _ => return None,
        })
    }
}

/// A type crossing the boundary.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Unit,
    Bool,
    I32,
    I64,
    U8,
    U32,
    F32,
    F64,
    /// `String` (owned) or `&str` (borrowed).
    String {
        borrowed: bool,
    },
    LinearColor,
    Vector2,
    Vector3,
    /// A `#[uenum]`, by its Rust name.
    Enum(String),
    /// `Obj<C>` (`optional`: `Option<Obj<C>>`), by the marker's Rust name.
    Obj {
        class: String,
        optional: bool,
    },
    /// `Vec<Obj<C>>` or `&[Obj<C>]`.
    ObjArray {
        class: String,
        borrowed: bool,
    },
    /// `Vec<T>` or `&[T]`.
    Slice {
        prim: Prim,
        borrowed: bool,
    },
}

/// Where a type is used, which decides what is allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Property,
    Param,
    Return,
}

fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    match ty {
        Type::Path(p) if p.qself.is_none() => p.path.segments.last(),
        _ => None,
    }
}

fn single_generic(seg: &syn::PathSegment) -> Option<&Type> {
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    }
}

/// The class marker of `Obj<C>`.
fn obj_class(ty: &Type) -> Option<String> {
    let seg = last_segment(ty)?;
    if seg.ident != "Obj" {
        return None;
    }
    let inner = last_segment(single_generic(seg)?)?;
    Some(inner.ident.to_string())
}

impl Ty {
    /// Recognizes `ty`, used as `usage`.
    pub fn parse(ty: &Type, usage: Use) -> Result<Ty, syn::Error> {
        let err = |msg: &str| syn::Error::new_spanned(ty, msg);
        if let Type::Tuple(t) = ty
            && t.elems.is_empty()
        {
            return if usage == Use::Return {
                Ok(Ty::Unit)
            } else {
                Err(err("`()` is only a return type"))
            };
        }
        if let Type::Reference(r) = ty {
            if usage != Use::Param {
                return Err(err(
                    "references are only allowed as parameters (`&str`, `&[T]`)",
                ));
            }
            if r.mutability.is_some() {
                return Err(err("Ferreal passes no `&mut` across the boundary"));
            }
            if let Type::Slice(s) = &*r.elem {
                if let Some(class) = obj_class(&s.elem) {
                    return Ok(Ty::ObjArray {
                        class,
                        borrowed: true,
                    });
                }
                let prim =
                    last_segment(&s.elem).and_then(|seg| Prim::from_name(&seg.ident.to_string()));
                return prim
                    .map(|prim| Ty::Slice {
                        prim,
                        borrowed: true,
                    })
                    .ok_or_else(|| err("slices are of `f32`, `u8`, `u32`, `i32` or `Obj<C>`"));
            }
            if last_segment(&r.elem).is_some_and(|s| s.ident == "str") {
                return Ok(Ty::String { borrowed: true });
            }
            return Err(err("the only references are `&str` and `&[T]`"));
        }
        let Some(seg) = last_segment(ty) else {
            return Err(err("Ferreal cannot pass this type to Unreal"));
        };
        let name = seg.ident.to_string();
        let plain = matches!(seg.arguments, PathArguments::None);
        let t = match name.as_str() {
            "bool" if plain => Ty::Bool,
            "i32" if plain => Ty::I32,
            "i64" if plain => Ty::I64,
            "u8" if plain => Ty::U8,
            "u32" if plain => Ty::U32,
            "f32" if plain => Ty::F32,
            "f64" if plain => Ty::F64,
            "String" if plain => Ty::String { borrowed: false },
            "LinearColor" if plain => Ty::LinearColor,
            "Vector2" if plain => Ty::Vector2,
            "Vector3" if plain => Ty::Vector3,
            "Obj" => Ty::Obj {
                class: obj_class(ty).ok_or_else(|| err("`Obj<C>` takes one class"))?,
                optional: false,
            },
            "Option" => {
                let inner =
                    single_generic(seg).ok_or_else(|| err("`Option<Obj<C>>` takes one type"))?;
                let class =
                    obj_class(inner).ok_or_else(|| err("the only `Option` is `Option<Obj<C>>`"))?;
                Ty::Obj {
                    class,
                    optional: true,
                }
            }
            "Vec" => {
                let inner = single_generic(seg).ok_or_else(|| err("`Vec<T>` takes one type"))?;
                if let Some(class) = obj_class(inner) {
                    Ty::ObjArray {
                        class,
                        borrowed: false,
                    }
                } else {
                    let prim =
                        last_segment(inner).and_then(|s| Prim::from_name(&s.ident.to_string()));
                    let prim = prim.ok_or_else(|| {
                        err("`Vec`s are of `f32`, `u8`, `u32`, `i32` or `Obj<C>`")
                    })?;
                    if usage == Use::Property {
                        return Err(err(
                            "number arrays are not properties yet; use them as parameters",
                        ));
                    }
                    Ty::Slice {
                        prim,
                        borrowed: false,
                    }
                }
            }
            _ if plain && name.chars().next().is_some_and(char::is_uppercase) => Ty::Enum(name),
            _ => {
                return Err(err(
                    "Ferreal cannot pass this type to Unreal (see ferreal_model::ty)",
                ));
            }
        };
        if usage == Use::Return && matches!(t, Ty::ObjArray { .. }) {
            return Err(err("object arrays are not returned yet"));
        }
        Ok(t)
    }

    /// The `#[repr(C)]` Rust type that carries this type from C++ to Rust (parameters
    /// C++ passes, property values it returns).
    pub fn rust_ffi_in(&self) -> String {
        match self {
            Ty::String { .. } => "::ferreal::ffi::Str16".into(),
            _ => self.rust_ffi_common(),
        }
    }

    /// The `#[repr(C)]` Rust type that carries this type from Rust to C++ (return values,
    /// property values Rust sets, arguments of C++ base methods).
    pub fn rust_ffi_out(&self) -> String {
        match self {
            Ty::String { .. } => "::ferreal::ffi::Str8".into(),
            _ => self.rust_ffi_common(),
        }
    }

    fn rust_ffi_common(&self) -> String {
        match self {
            Ty::Unit => "()".into(),
            Ty::Bool => "bool".into(),
            Ty::I32 => "i32".into(),
            Ty::I64 => "i64".into(),
            Ty::U8 | Ty::Enum(_) => "u8".into(),
            Ty::U32 => "u32".into(),
            Ty::F32 => "f32".into(),
            Ty::F64 => "f64".into(),
            Ty::String { .. } => unreachable!(),
            Ty::LinearColor => "::ferreal::LinearColor".into(),
            Ty::Vector2 => "::ferreal::Vector2".into(),
            Ty::Vector3 => "::ferreal::Vector3".into(),
            Ty::Obj { .. } => "::ferreal::ffi::RawObj".into(),
            Ty::ObjArray { .. } => "::ferreal::ffi::Slice<::ferreal::ffi::RawObj>".into(),
            Ty::Slice { prim, .. } => format!("::ferreal::ffi::Slice<{}>", prim.rust()),
        }
    }

    /// The C++ type of the same carrier, as the generated C++ spells it (in namespace
    /// `Ferreal`); `inbound` as [`Ty::rust_ffi_in`], else [`Ty::rust_ffi_out`].
    pub fn cpp_ffi(&self, inbound: bool) -> String {
        match self {
            Ty::Unit => "void".into(),
            Ty::Bool => "bool".into(),
            Ty::I32 => "int32".into(),
            Ty::I64 => "int64".into(),
            Ty::U8 | Ty::Enum(_) => "uint8".into(),
            Ty::U32 => "uint32".into(),
            Ty::F32 => "float".into(),
            Ty::F64 => "double".into(),
            Ty::String { .. } if inbound => "Ferreal::FStr16".into(),
            Ty::String { .. } => "Ferreal::FStr8".into(),
            Ty::LinearColor => "Ferreal::FColor4".into(),
            Ty::Vector2 => "Ferreal::FVec2".into(),
            Ty::Vector3 => "Ferreal::FVec3".into(),
            Ty::Obj { .. } => "void*".into(),
            Ty::ObjArray { .. } => "Ferreal::TSlice<void*>".into(),
            Ty::Slice { prim, .. } => format!("Ferreal::TSlice<{}>", prim.cpp()),
        }
    }

    /// Whether values of this type are copied as they are (no conversion on either side).
    pub fn is_plain(&self) -> bool {
        matches!(
            self,
            Ty::Unit | Ty::Bool | Ty::I32 | Ty::I64 | Ty::U8 | Ty::U32 | Ty::F32 | Ty::F64
        )
    }

    /// The Rust class markers this type refers to.
    pub fn class(&self) -> Option<&str> {
        match self {
            Ty::Obj { class, .. } | Ty::ObjArray { class, .. } => Some(class),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(s: &str, usage: Use) -> Result<Ty, syn::Error> {
        Ty::parse(&syn::parse_str(s).unwrap(), usage)
    }

    #[test]
    fn recognizes() {
        assert_eq!(ty("f32", Use::Property).unwrap(), Ty::F32);
        assert_eq!(
            ty("ferreal::LinearColor", Use::Property).unwrap(),
            Ty::LinearColor
        );
        assert_eq!(
            ty("&str", Use::Param).unwrap(),
            Ty::String { borrowed: true }
        );
        assert_eq!(
            ty("Option<Obj<FontFace>>", Use::Property).unwrap(),
            Ty::Obj {
                class: "FontFace".into(),
                optional: true
            }
        );
        assert_eq!(
            ty("Vec<Obj<FontFace>>", Use::Property).unwrap(),
            Ty::ObjArray {
                class: "FontFace".into(),
                borrowed: false
            }
        );
        assert_eq!(
            ty("&[f32]", Use::Param).unwrap(),
            Ty::Slice {
                prim: Prim::F32,
                borrowed: true
            }
        );
        assert_eq!(
            ty("Align", Use::Property).unwrap(),
            Ty::Enum("Align".into())
        );
        assert_eq!(ty("()", Use::Return).unwrap(), Ty::Unit);
    }

    #[test]
    fn rejects() {
        assert!(ty("&str", Use::Property).is_err());
        assert!(ty("&mut [f32]", Use::Param).is_err());
        assert!(ty("Vec<f32>", Use::Property).is_err());
        assert_eq!(
            ty("Vec<u8>", Use::Return).unwrap(),
            Ty::Slice {
                prim: Prim::U8,
                borrowed: false
            }
        );
        assert!(ty("Vec<Obj<C>>", Use::Return).is_err());
        assert!(ty("HashMap<u32, u32>", Use::Param).is_err());
        assert!(ty("()", Use::Param).is_err());
    }
}
