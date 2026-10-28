//! Write Unreal Engine classes in Rust.
//!
//! A `#[uclass]` struct becomes a `UCLASS`: its `#[uproperty]` fields are `UPROPERTY`s stored
//! on the Unreal object (so the editor, serialization and Blueprints see them as usual), its
//! other fields are Rust state, and the methods of its `#[uclass_impl]` block become
//! `UFUNCTION`s, overrides of the C++ parent's virtual functions, and change notifications.
//!
//! ```ignore
//! use rivet::prelude::*;
//!
//! #[uclass(parent = "UObject", include = "UObject/Object.h", BlueprintType)]
//! pub struct Greeter {
//!     #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Greeting", default = "World")]
//!     name: String,
//!     greeted: u32,
//! }
//!
//! #[uclass_impl]
//! impl Greeter {
//!     #[ufunction(BlueprintCallable, category = "Greeting")]
//!     fn greet(&mut self) -> String {
//!         self.greeted += 1;
//!         format!("Hello, {} ({})", self.name(), self.greeted)
//!     }
//! }
//! ```
//!
//! The macros write the Rust half of the glue; `rivet-build`, from the crate's `build.rs`,
//! reads the same items and writes the C++ half (the `UCLASS` headers and their
//! implementation) into the Unreal module. The Rust compiles to a static library the module
//! links, so it works in modular and monolithic builds alike.
//!
//! What crosses the boundary is listed in `rivet_model::ty`. Calls go through
//! `extern "C"` functions only; panics are caught at the boundary and logged.
//!
//! Engine classes are bound on demand: `rivet::engine!(AActor, UGameplayStatics);` names the
//! classes a crate calls, and `rivet-build` generates their reflected functions as typed
//! Rust (an `<Class>Api` trait per class, in the crate's `engine` module) from the reflection
//! data Unreal Header Tool exports (the Rivet plugin's exporter). Each call fills a C++
//! struct of its parameters and goes through `ProcessEvent`; the struct is checked against
//! the engine's reflection the first time.

pub mod ffi;
pub mod log;
mod obj;
mod types;

pub use obj::{AsObject, Class, Derives, IsA, Obj, obj_from_ffi, objs_from_ffi};
pub use rivet_macros::{
    cpp_base, engine, engine_class, export, module, uclass, uclass_impl, uenum,
};
pub use types::{LinearColor, Rotator, UEnum, Vector2, Vector3};

pub mod prelude {
    pub use crate::{
        AsObject, IsA, LinearColor, Obj, Rotator, UEnum, Vector2, Vector3, cpp_base, engine,
        engine_class, export, uclass, uclass_impl, uenum,
    };
}
