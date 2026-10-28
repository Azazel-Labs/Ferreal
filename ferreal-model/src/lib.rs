//! Ferreal's model of the Rust items it exposes to Unreal Engine.
//!
//! `ferreal-macros` reads these items while compiling (and writes the Rust half of the glue),
//! `ferreal-build` reads them from the sources in `build.rs` (and writes the C++ half). Both
//! parse them here, and take every name and symbol from [`names`], so the two halves always
//! agree.

pub mod attr;
pub mod items;
pub mod names;
pub mod ty;

pub use attr::{Arg, ArgValue, DefaultValue, parse_args};
pub use items::{
    Base, BaseMethod, CallSuper, Class, ClassImpl, EngineClass, EngineList, Enum, Export, Method,
    MethodKind, Param, Property, Receiver, Variant, base_marker, path_name,
};
pub use names::{Symbols, pascal};
pub use ty::{Prim, Ty, Use};

/// The environment variable `ferreal-build` sets for the crate it builds (with
/// `cargo:rustc-env`): the Unreal module's name, which prefixes every symbol.
pub const MODULE_ENV: &str = "FERREAL_MODULE";
