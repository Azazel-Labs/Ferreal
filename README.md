# Rivet

Write Unreal Engine 5 classes in Rust. Rivet turns attributes on Rust items into
`UCLASS`es, `UPROPERTY`s, `UFUNCTION`s and `UENUM`s, and generates the C++ that binds
them, as readable source in your module.

```rust
use rivet::prelude::*;

rivet::module!();

#[uclass(parent = "UObject", include = "UObject/Object.h", BlueprintType)]
pub struct Greeter {
    /// Who to greet.
    #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Greeting", default = "World")]
    name: String,
    /// Rust state, not seen by Unreal.
    greeted: u32,
}

#[uclass_impl]
impl Greeter {
    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn greet(&mut self, punctuation: &str) -> String {
        self.greeted += 1;
        format!("Hello, {}{punctuation} #{}", self.name(), self.greeted)
    }
}
```

That is a `UGreeter` in Blueprints and C++, with an editable `Name` and a `Greet` node.

## How it works

Rust is the source of truth, and there are two halves of glue, both derived from the same
items by one shared parser (`rivet-model`), so they cannot disagree:

* **The Rust half** comes from the attribute macros (`rivet-macros`, through `rivet`): the
  property accessors, the `extern "C"` functions C++ calls, the tables of C++ functions Rust
  calls.
* **The C++ half** comes from `rivet-build`, which the crate's `build.rs` runs: it reads the
  crate's sources and writes, into the Unreal module, a header per class (a real `UCLASS`
  that Unreal Header Tool processes like any other), the `UENUM`s, and one `.cpp` with
  everything they do.

The crate compiles to a **static library** that the module links, so the Rust is part of the
module's binary: modular (editor) and monolithic (packaged game) builds work the same way,
and nothing is loaded at run time.

```
Rust crate ──cargo──> static library ──────────────┐
    │                                              ├─> the module's binary
    └─build.rs (rivet-build)─> Public/Rivet/*.h ───┤
                               Private/Rivet/*.cpp ┘   (UHT reads the headers)
```

The module's `Build.cs` runs cargo before Unreal Header Tool reads the headers (see
[examples/hello](examples/hello/Source/HelloRust/HelloRust.Build.cs): its "Rust" region is
the part to copy), and tells Unreal Build Tool to run again when the Rust changes.

What lives where:

* **Properties live on the Unreal object.** A `#[uproperty]` field becomes a `UPROPERTY`
  member of the C++ class, so the editor, serialization, Blueprints, replication and
  garbage collection see it as usual. Rust reads and writes it through generated methods:
  `self.name()`, `self.set_name("x")`.
* **Rust state lives in Rust.** The other fields form a Rust struct per object, created on the
  object's first call into Rust (`Default::default()`, then `#[init]`), dropped in
  `BeginDestroy`. It sits in a `RefCell`: a call that would re-enter an object already
  borrowed mutably (C++ calling back into it) is refused and logged instead of aliasing.
* **Panics stop at the boundary.** Every call from C++ is wrapped: a panic is logged to
  `LogRivet<Module>` with its location, and C++ gets a default value.
* **Calls are plain C.** Rust calls C++ through tables of function pointers that the generated
  `Rivet::<Module>::Register()` hands over (run before the first call into Rust); C++ calls
  Rust through `extern "C"` functions named `rivet_<Module>_...`.

## The attributes

| Attribute | On | Becomes |
|---|---|---|
| `#[uclass(parent = "UCpp", include = "Cpp.h", ...)]` | a struct | a `UCLASS` deriving from `UCpp` |
| `#[uproperty(...)]` | a field of it | a `UPROPERTY` |
| `#[uclass_impl]` | its `impl` block | (marks the methods below) |
| `#[ufunction(...)]` | a method | a `UFUNCTION` (`&self`: `const`; no receiver: `static`) |
| `#[uoverride]` | a method | an override of a virtual function of the C++ parent |
| `#[on_changed]` | `fn(&mut self, p: <Class>Property)` | called when a property changes (generated setters, the editor) |
| `#[init]` | `fn(&mut self)` | called once a new object's Rust state exists |
| `#[uenum]` | a fieldless enum | a `UENUM` (`enum class E<Name> : uint8`) |
| `#[cpp_base(class = "UCpp", include = "Cpp.h")]` | a trait | methods of a hand-written C++ class that Rust calls |
| `engine_class!(pub Name = "UCpp" in "Cpp.h");` | | a handle type for `Obj<Name>` |
| `rivet::engine!(AActor, UGameplayStatics);` | | typed bindings of those engine classes (see below) |
| `#[export]` | a free function | `Rivet::<Module>::<Name>(...)`, callable from C++ |
| `rivet::module!();` | once per crate | the module's entry point |

Arguments, beyond Unreal's own specifiers (any flag passes through: `EditAnywhere`,
`BlueprintType`...):

* `#[uclass]`: `parent` (required), `include` (repeatable), `cpp_name` (default: `U` or `A`,
  as the parent's, then the struct's name), `bases(Trait, ...)` (the `#[cpp_base]` traits the
  parent implements), `meta(...)`, `category`, `class_group = "..."`, `config = "..."`,
  `raw = "..."` (a specifier as written).
* `#[uproperty]`: `default = ...` (a literal, `[r, g, b, a]` for colors and vectors, a variant
  for enums), `setter` (a `BlueprintSetter` that notifies `#[on_changed]`), `name = "..."`,
  `category`, `meta(...)`.
* `#[ufunction]`: `name = "..."`, `category`, `meta(...)`.
* `#[uoverride]`: `const`, `call_super = "before" | "after" | "never"` (default `before` for
  functions returning nothing, else `never`), `name = "..."`.
* `#[uenum]`: `cpp_name`; variants take `#[umeta(DisplayName = "...")]`.
* `#[cpp_base]` methods: `#[cpp(name = "...")]`.

C++ names are the Rust names in PascalCase (`font_size` → `FontSize`, booleans `bAutowrap`),
parameters with `In` in front (Unreal treats shadowed names as errors).

## Types

| Rust | Unreal |
|---|---|
| `bool`, `i32`, `i64`, `u8`, `u32`, `f32`, `f64` | `bool`, `int32`, `int64`, `uint8`, `uint32`, `float`, `double` |
| `String`, `&str` (parameters) | `FString` (UTF-16 ⇄ UTF-8 at the boundary) |
| `LinearColor`, `Vector2`, `Vector3` | `FLinearColor`, `FVector2D`, `FVector` |
| a `#[uenum]` | its `UENUM` |
| `Obj<C>`, `Option<Obj<C>>` | `C*`, `TObjectPtr<C>` |
| `Vec<Obj<C>>`, `&[Obj<C>]` | `TArray<TObjectPtr<C>>` (properties), `TArray<C*>` |
| `Vec<T>`, `&[T]` (`T`: `f32`, `u8`, `u32`, `i32`; parameters) | `TArray<T>` (`UFUNCTION`s), `TConstArrayView<T>` |

`Obj<C>` does not keep its object alive: Unreal's garbage collector owns it. A handle
received in a call is valid for the call; keep one longer only when a `UPROPERTY` references
the object.

## Talking to C++ you write

Most engine work stays in C++: a `#[uclass]` derives from a C++ class you write (a widget, a
component), whose virtual functions Rust overrides with `#[uoverride]` and whose methods Rust
calls through a `#[cpp_base]` trait:

```rust
#[cpp_base(class = "UMyWidgetBase", include = "MyWidgetBase.h")]
pub trait MyWidgetBase {
    /// Calls UMyWidgetBase::SetQuads(TConstArrayView<float>).
    fn set_quads(&self, quads: &[f32]);
}

#[uclass(parent = "UMyWidgetBase", include = "MyWidgetBase.h", bases(MyWidgetBase))]
pub struct MyWidget { /* ... */ }
```

`self.set_quads(&data)` then calls the C++ method on the object. The generated `.cpp`
calls it by name, so a signature that doesn't fit is a C++ compile error that points at it.

## Calling engine classes

Name the engine classes a crate calls, by their C++ names, and their reflected functions
become typed Rust, generated from Unreal's own reflection data:

```rust
rivet::engine!(AActor, UPrimitiveComponent, UKismetStringLibrary);

use engine::{ActorApi, CollisionEnabled, KismetStringLibrary, PrimitiveComponentApi};

fn tidy(actor: Obj<engine::Actor>, mesh: Obj<engine::PrimitiveComponent>) -> String {
    actor.set_actor_hidden_in_game(true);
    mesh.set_collision_enabled(CollisionEnabled::QUERY_ONLY);
    KismetStringLibrary::concat_str_str(&actor.get_actor_name_or_label(), " (hidden)")
}
```

Only what a crate names is generated, with the classes those need: in the crate's `engine`
module (which `rivet::module!()` declares), a marker type per class, an `<Class>Api` trait
of its functions implemented by every handle of a class that `IsA` it (so a
`PrimitiveComponent` has `SceneComponentApi` too, and a `#[uclass]` deriving from `AActor`
has `ActorApi`), its static functions on the marker, and newtypes of the enums they take.
`K2_` prefixes go; snake_case names, `bFlag` parameters as `flag`; object parameters take any
`Obj<C>` whose class `IsA` the one asked for (`.upcast()` converts explicitly). Classes the
signatures mention get markers, so they can be passed and returned; name them to call theirs.

**Where the data comes from.** The Rivet plugin ([unreal/Rivet](unreal/Rivet)) holds an
exporter for Unreal Header Tool: during every Unreal build it writes the reflection data of
every class and enum UHT parsed (engine, project, plugins) to `RivetEngine.json`. The
Build.cs template finds it and passes it to cargo (`RIVET_ENGINE_METADATA`). The part a
crate uses is kept next to its `Cargo.toml` in `rivet-engine.json`: commit it, and a fresh
checkout (or a build machine outside Unreal) builds without the export. Naming a class not
exported yet takes one more build: the first exports it (the template reports this and lets
the build go on), the next generates its bindings.

**How calls are made.** Each call goes through a generated C++ thunk that fills a struct of
the function's parameters, laid out by the C++ compiler, and calls `ProcessEvent`, as
Unreal's own generated event thunks do. Calling through reflection needs no export from the
engine's DLLs, works in modular and monolithic builds alike, and reaches Blueprint overrides
of events. Before the first call, the struct is checked against the function's reflection:
the end of its parameters, and each parameter's name, offset, size and property class (and
struct, for structs). A function that does not match (bindings stale against the engine) or
does not exist in the build (an editor-only function in a game) is disabled, with an error in
the log, and returns zeros: Rivet never writes bytes at an offset it has not checked. A call
on an object that is not of the class is refused the same way.

**Editor support.** rust-analyzer runs the build script and expands the macros, so the
bindings complete, type-check and show the engine's tooltips on hover like any Rust code;
each item also carries its C++ name as a doc alias (`AActor`, `K2_SetActorLocation`), and
`cargo doc` renders them. Outside an Unreal build, the build script reads the lock file; to
see a class you have just named before the next Unreal build, point rust-analyzer at the
exporter's output: `"rust-analyzer.cargo.extraEnv": { "RIVET_ENGINE_METADATA":
"<Project>/Plugins/Rivet/Intermediate/Build/<Platform>/<Target>/Inc/RivetMetadata/UHT/RivetEngine.json" }`.
The generated C++ sits in the module's `Source`, so the IDE project Unreal generates indexes
it too.

Parameters: the types above, `FName` (as `&str`/`String`), `FRotator` (`Rotator`) and enums
of one byte. Functions with output parameters (non-const references), arrays, other structs,
`FText`, class references, soft and weak references, delegates or interfaces are left out
for now; the generated file lists them with why.

## Setting up a module

1. A Rust crate with `crate-type = ["staticlib"]`, depending on `rivet` (and on
   `rivet-build` as a build dependency).
2. Its `build.rs`:
   ```rust
   fn main() {
       rivet_build::Module::new("MyModule").output("../Source/MyModule").run();
   }
   ```
3. `rivet::module!();` in its `lib.rs`.
4. The module's `Build.cs`: the "Rust" region of
   [HelloRust.Build.cs](examples/hello/Source/HelloRust/HelloRust.Build.cs), and a call to
   `BuildRust(...)`. Add a `[profile.unreal]` (release with `panic = "unwind"`) to the
   workspace's `Cargo.toml`, as this repository's.
5. For `rivet::engine!`: copy [unreal/Rivet](unreal/Rivet) into the project's `Plugins`
   (Unreal Build Tool builds its exporter, a `.ubtplugin.csproj`, with the project).

The generated files can be committed (their diffs show what changed in the C++ API); the
build writes them again either way, and removes those it no longer generates.

## Testing without Unreal

`tests/mock-ue/run.sh` builds the example, compiles its C++ (the hand-written parent and the
generated glue) against minimal stand-ins for the engine headers it uses, links the Rust and
runs a test that drives the class through the glue: properties of every type, functions,
overrides, Rust calling C++, change notifications, a caught panic, destruction. It also calls the
engine bindings of the example's `rivet::engine!` against a mock engine whose reflection lays
parameters out the way the engine links functions, and whose natives read them through those
offsets: every kind of parameter and return value, static functions, and a function the mock
engine declares differently, which the check must disable. The reflection data it builds
from (`tests/mock-ue/RivetEngine.json`) is the exporter's own output:
`tests/exporter/run.sh` runs the exporter's source over a made-up session of Unreal Header
Tool, against a stand-in for UHT's API (it needs the .NET SDK).

These check the glue's C++, its ABI, and the exporter's logic, not Unreal's own tools
(Unreal Header Tool, Unreal Build Tool, UHT's real API), which only a real engine build
exercises.

## Limitations

* Rivet has not yet been through a real Unreal build: the generated code follows UE 5.x
  conventions and passes the mock test, and the first engine build may need small fixes.
  The exporter compiles against a stand-in of UHT's API, so a real build may also find
  names to adjust there.
* Engine bindings go through `ProcessEvent`: a lookup and a check per call on top of the
  function, fine for gameplay calls, not for per-element work in a hot loop (write that in a
  `#[cpp_base]` method, or in Rust).
* Two Rivet modules in one **monolithic** binary each carry the Rust standard library, and
  the linker reports duplicate symbols. Until Rivet links one shared Rust library per
  binary, put Rust in one module per game (editor builds, one DLL per module, are fine).
* No hot reload or Live Coding of Rust: rebuild the module.
* Structs (`USTRUCT`), delegates, interfaces and maps are not types yet; pass their parts, or
  wrap them in your C++ base class.
* Consoles need their platform's Rust target, which the platform holders' SDKs do not provide
  publicly (see the Build.cs: add the triple once you have one).
