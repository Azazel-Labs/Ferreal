//! Write Unreal Engine classes in Rust.
//!
//! A `#[uclass]` struct becomes a `UCLASS`: its `#[uproperty]` fields are `UPROPERTY`s stored
//! on the Unreal object (so the editor, serialization and Blueprints see them as usual), its
//! other fields are Rust state, and the methods of its `#[uclass_impl]` block become
//! `UFUNCTION`s, overrides of the C++ parent's virtual functions, and change notifications.
//!
//! ```ignore
//! use ferreal::prelude::*;
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
//! The macros write the Rust half of the glue; `ferreal-build`, from the crate's `build.rs`,
//! reads the same items and writes the C++ half (the `UCLASS` headers and their
//! implementation) into the Unreal module. The Rust compiles to a static library the module
//! links, so it works in modular and monolithic builds alike.
//!
//! What crosses the boundary is listed in `ferreal_model::ty`. Calls go through
//! `extern "C"` functions only; panics are caught at the boundary and logged.
//!
//! Engine classes are bound on demand: `ferreal::engine!(AActor, UGameplayStatics);` names the
//! classes a crate calls, and `ferreal-build` generates their reflected functions as typed
//! Rust (an `<Class>Api` trait per class, in the crate's `engine` module) from the reflection
//! data Unreal Header Tool exports (the Ferreal plugin's exporter). Each call fills a C++
//! struct of its parameters and goes through `ProcessEvent`; the struct is checked against
//! the engine's reflection the first time.
//!
//! Anything else in C++ is written inline: `cpp!([actor: Obj<engine::Actor>] -> String {
//! return actor->GetActorNameOrLabel(); })` is compiled into the module (by `ferreal-build`)
//! and called directly.

pub mod ffi;
pub mod log;
mod obj;
mod types;

pub use ferreal_macros::{
    cpp, cpp_base, cpp_include, engine, engine_class, export, module, uclass, uclass_impl, uenum,
};
pub use obj::{AsObject, Class, ClassOf, Derives, IsA, Obj, obj_from_ffi, objs_from_ffi};
pub use types::{
    Box3, Carry, Color, DateTime, IntPoint, IntVector, LinearColor, Matrix, Quat, Rotator,
    Timespan, Transform, UEnum, Vector2, Vector3, Vector4,
};

pub mod prelude {
    pub use crate::{
        AsObject, ClassOf, IsA, LinearColor, Obj, Quat, Rotator, Transform, UEnum, Vector2,
        Vector3, cpp, cpp_base, engine, engine_class, export, uclass, uclass_impl, uenum,
    };
}
