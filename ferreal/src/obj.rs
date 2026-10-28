//! Handles of Unreal objects, typed by marker types (`engine_class!` declares them for C++
//! classes, `#[uclass]` structs are their own).

use std::fmt;
use std::marker::PhantomData;

use crate::ffi::RawObj;

/// A class Rust can hold handles of.
pub trait Class: 'static {
    /// Its C++ name (`UFontFace`).
    const CPP: &'static str;
}

/// `Self` derives from a C++ class with a `#[cpp_base]` trait: `B` is `dyn Trait`, and `Self` can
/// call the trait's methods.
/// `engine_class!(..., bases(...))` and `#[uclass(bases(...))]` implement it.
pub trait Derives<B: ?Sized> {}

/// `Self` is the class `C` or derives from it: what the generated engine bindings
/// (`ferreal::engine!`) use to accept a handle where Unreal accepts a `C*`, and to give a class
/// the methods of its ancestors. The bindings implement it for the engine classes and their
/// ancestors, for `#[uclass]` structs and for `engine_class!` handles of reflected classes.
pub trait IsA<C> {}

/// Something backed by an Unreal object: an `Obj`, or a `#[uclass]` struct (its object).
pub trait AsObject {
    fn raw_object(&self) -> RawObj;
}

/// A handle of an Unreal object of class `C` (never null; `Option<Obj<C>>` for one that may
/// be).
///
/// It does not keep the object alive: Unreal's garbage collector owns it. A handle received
/// in a call is valid for that call; keep one longer only when something on the Unreal side
/// (a `UPROPERTY`) keeps the object referenced.
pub struct Obj<C> {
    raw: RawObj,
    class: PhantomData<fn() -> C>,
}

impl<C> Obj<C> {
    /// The handle of `raw`, `None` for null.
    pub fn from_raw(raw: RawObj) -> Option<Obj<C>> {
        (!raw.is_null()).then_some(Obj {
            raw,
            class: PhantomData,
        })
    }

    /// The `UObject*`.
    pub fn raw(self) -> RawObj {
        self.raw
    }

    /// The same object as a handle of another class. Unchecked: the caller knows the class.
    pub fn cast_unchecked<D>(self) -> Obj<D> {
        Obj {
            raw: self.raw,
            class: PhantomData,
        }
    }
}

impl<C> Clone for Obj<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Obj<C> {}

impl<C> PartialEq for Obj<C> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<C> Eq for Obj<C> {}

impl<C> std::hash::Hash for Obj<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<C> fmt::Debug for Obj<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Obj({:p})", self.raw.0)
    }
}

impl<C> AsObject for Obj<C> {
    fn raw_object(&self) -> RawObj {
        self.raw
    }
}

impl<C: Derives<B>, B: ?Sized> Derives<B> for Obj<C> {}

impl<C: IsA<D>, D> IsA<D> for Obj<C> {}

impl<C> Obj<C> {
    /// The same object as a handle of an ancestor class (checked by the types).
    pub fn upcast<D>(self) -> Obj<D>
    where
        C: IsA<D>,
    {
        self.cast_unchecked()
    }
}

/// Converts an object handle received from C++ (generated glue).
pub fn obj_from_ffi<C>(raw: RawObj, what: &str) -> Obj<C> {
    Obj::from_raw(raw)
        .unwrap_or_else(|| panic!("{what}: C++ passed a null object (use Option<Obj<C>>)"))
}

/// The handles of an array received from C++, without the nulls.
///
/// # Safety
/// `slice` comes from the generated glue (a live array for the call).
pub unsafe fn objs_from_ffi<C>(slice: crate::ffi::Slice<RawObj>) -> Vec<Obj<C>> {
    // SAFETY: the caller's contract.
    unsafe { slice.as_slice() }
        .iter()
        .filter_map(|&r| Obj::from_raw(r))
        .collect()
}
