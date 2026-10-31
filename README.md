# Ferreal

*Ferrous + Unreal: Unreal Engine, in Rust, fer real.*

Write Unreal Engine 5 classes in Rust. Ferreal turns attributes on Rust items into
`UCLASS`es, `UPROPERTY`s, `UFUNCTION`s and `UENUM`s, and generates the C++ that binds
them, as readable source in your module.

```rust
use ferreal::prelude::*;

ferreal::module!();

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
items by one shared parser (`ferreal-model`), so they cannot disagree:

* **The Rust half** comes from the attribute macros (`ferreal-macros`, through `ferreal`): the
  property accessors, the `extern "C"` functions C++ calls, the tables of C++ functions Rust
  calls.
* **The C++ half** comes from `ferreal-build`, which the crate's `build.rs` runs: it reads the
  crate's sources and writes, into the Unreal module, a header per class (a real `UCLASS`
  that Unreal Header Tool processes like any other), the `UENUM`s, and one `.cpp` with
  everything they do.

The crate compiles to a **static library** that the module links, so the Rust is part of the
module's binary: modular (editor) and monolithic (packaged game) builds work the same way,
and nothing is loaded at run time.

```
Rust crate ──cargo──> static library ──────────────┐
    │                                              ├─> the module's binary
    └─build.rs (ferreal-build)─> Public/Ferreal/*.h ───┤
                               Private/Ferreal/*.cpp ┘   (UHT reads the headers)
```

The module's `Build.cs` runs cargo before Unreal Header Tool reads the headers (see
[FerrealSample.Build.cs](examples/UnrealGame/Plugins/FerrealSample/Source/FerrealSample/FerrealSample.Build.cs):
its "Rust" region is the part to copy), and tells Unreal Build Tool to run again when the
Rust changes. The module is usually a plugin's, which the game's C++ and Blueprints use as they
would a C++ plugin: its classes are ordinary `UCLASS`es, its headers ordinary headers.

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
  `LogFerreal<Module>` with its location, and C++ gets a default value.
* **Calls are plain C.** Rust calls C++ through tables of function pointers that the generated
  `Ferreal::<Module>::Register()` hands over (run before the first call into Rust); C++ calls
  Rust through `extern "C"` functions named `ferreal_<Module>_...`.

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
| `ferreal::engine!(AActor, UGameplayStatics);` | | typed bindings of those engine classes (see below) |
| `#[export]` | a free function | `Ferreal::<Module>::<Name>(...)`, callable from C++ |
| `ferreal::module!();` | once per crate | the module's entry point |
| `cpp!([captures] -> Type { C++ })` | an expression | C++ compiled into the module and called (see below) |

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

## C++ inline: `cpp!`

Any C++ the module can compile can be written where it is used, in a `cpp!` block: plain
engine methods, inline functions, templates, `FMath`, your own C++.

```rust
fn label(actor: Obj<engine::Actor>, times: i32) -> String {
    cpp!([actor: Obj<engine::Actor>, times: i32] -> String {
        // GetActorNameOrLabel is a plain C++ method, not a UFUNCTION.
        return FString::Printf(TEXT("%s x%d"), *actor->GetActorNameOrLabel(), FMath::Max(times, 2));
    })
}
```

The captures are Rust variables with their types, as Ferreal passes them (see Types), or of
the engine's structs and enums (`engine::HitResult`), `Rotator`, `Quat` and `Transform`; in
the C++ they are variables of the same names holding the C++ values (`AActor* actor`,
`int32 times`, an `FString` for a string, an `FHitResult`, an engine enum as its integer).
Structs and enums are borrowed, not moved, as a closure would. The block is the body of a C++ function that
returns the C++ value of the `-> Type` (nothing without one). `ferreal-build` copies each
block, comments and all, into the module's `.cpp` as a function of its own, with the headers
of the classes it captures (`ferreal::cpp_include!("Header.h");` adds others), and Rust calls
it directly: no reflection, no `ProcessEvent`. A mistake in the C++ is a C++ compile error in
that function, whose comment gives the block's place in the Rust.

The C++ goes in braces when it is also valid Rust tokens, which almost all C++ is; in a string
otherwise (`cpp!([n: i32] -> i32 r#"return n * 1'000;"#)`, for digit separators or
multi-character literals). Blocks are found in the crate's sources by `ferreal-build`,
anywhere (inside other macros' arguments too), except in `macro_rules!` definitions that
substitute `$` variables into them. The C++ runs only in the Unreal module (and the mock test), never in a cargo
build, and C++ is not checked like Rust: a captured object is `nullptr` if it is not of the
class (`Cast`), and what the C++ does with it is up to the C++.

## Calling engine classes

Name the engine classes a crate calls, by their C++ names, and their reflected functions
become typed Rust, generated from Unreal's own reflection data:

```rust
ferreal::engine!(AActor, UPrimitiveComponent, UKismetStringLibrary, UKismetSystemLibrary);

use engine::{ActorApi, CollisionEnabled, KismetStringLibrary, KismetSystemLibrary, PrimitiveComponentApi};

fn tidy(actor: Obj<engine::Actor>, mesh: Obj<engine::PrimitiveComponent>) -> String {
    actor.set_actor_hidden_in_game(true);
    mesh.set_collision_enabled(CollisionEnabled::QUERY_ONLY);
    let name = KismetSystemLibrary::get_display_name(Some(actor));
    KismetStringLibrary::concat_str_str(&name, " (hidden)")
}
```

Only what a crate names is generated, with the classes those need: in the crate's `engine`
module (which `ferreal::module!()` declares), a marker type per class, an `<Class>Api` trait
of its functions implemented by every handle of a class that `IsA` it (so a
`PrimitiveComponent` has `SceneComponentApi` too, and a `#[uclass]` deriving from `AActor`
has `ActorApi`), its static functions on the marker, and newtypes of the enums they take.
`K2_` prefixes go; snake_case names, `bFlag` parameters as `flag`; object parameters take any
`Obj<C>` whose class `IsA` the one asked for (`.upcast()` converts explicitly). Classes the
signatures mention get markers, so they can be passed and returned; name them to call theirs.

**Where the data comes from.** A crate's build writes Ferreal's exporter for Unreal Header
Tool ([ferreal-build/exporter](ferreal-build/exporter)) beside its Unreal module, in the
plugin: `Source/<Module>Exporter`, a plugin of Unreal Build Tool (`.ubtplugin.csproj`), which
UBT builds with the project. During every build it writes the reflection data of every class
and enum UHT parsed (engine, project, plugins) to
`<Project>/Intermediate/Ferreal/FerrealEngine.json`, and the Build.cs template passes that to
cargo (`FERREAL_ENGINE_METADATA`). Each plugin's copy is named after its module, and the first
to run in a build exports for all. The part a crate uses is kept next to its `Cargo.toml` in
`ferreal-engine.json`, a snapshot of the engine it was built with, which the build reads
outside Unreal (rust-analyzer, `cargo check`).

**One build, always.** The Build.cs runs cargo before Unreal Header Tool exports the data: in
a fresh checkout it has none, and a crate calling a `UFUNCTION` added to C++ in the same change
has old data. Then the template does not stop the build. It writes a `FerrealCargoFailed.cpp`
holding an `#error`, and leaves the cargo build to the exporter
(`Intermediate/Ferreal/Pending`), which runs it once it has exported the data, after Unreal
Header Tool has read the headers and before the C++ compiles; if that succeeds, the `#error`
goes. Meanwhile the crate's build writes all it can without the data: the headers Unreal
Header Tool reads, and, empty, the sources the exporter's build fills in (Unreal Build Tool
lists a module's sources before then). What Unreal Header Tool reads does not depend on the
data: a header includes the headers of the engine classes it passes, and without the data
declares them ahead instead (`class AActor;`), or keeps the includes the last build wrote;
the exporter's build writes the includes, which Unreal Header Tool does not read. There is a
source per class `ferreal::engine!` names, whether the crate calls its functions or not
(those of shared types' classes go in `Ferreal<Module>Engine.cpp`). A crate of shared types
without the data hands on none, and fails to compile instead, so that the crates depending on
it write theirs. A build without new data runs cargo once, as before. A Rust error stays an
error: the build fails with cargo's errors.

**Dependencies.** With the data, ferreal-build lists the modules of the engine classes,
structs and enums the crate uses in `Private/Ferreal/FerrealModules.txt`, which the Build.cs
template adds to the module's dependencies, and adds their plugins to the `Plugins` of the
plugin's `.uplugin` (EnhancedInput for `UInputMappingContext`). Unreal enables plugins before a
Build.cs runs: if the project does not enable a plugin newly added there, the build that adds
it fails, and the next one has it. Commit both files with the glue.

**How calls are made.** Each call goes through a generated C++ thunk that fills a struct of
the function's parameters, laid out by the C++ compiler, and calls `ProcessEvent`, as
Unreal's own generated event thunks do. Calling through reflection needs no export from the
engine's DLLs, works in modular and monolithic builds alike, and reaches Blueprint overrides
of events. Before the first call, the struct is checked against the function's reflection:
the end of its parameters, and each parameter's name, offset, size and property class (and
struct, for structs). A function that does not match (bindings stale against the engine) or
does not exist in the build (an editor-only function in a game) is disabled, with an error in
the log, and returns zeros: Ferreal never writes bytes at an offset it has not checked. A call
on an object that is not of the class is refused the same way.

**Editor support.** rust-analyzer runs the build script and expands the macros, so the
bindings complete, type-check and show the engine's tooltips on hover like any Rust code;
each item also carries its C++ name as a doc alias (`AActor`, `K2_SetActorLocation`), and
`cargo doc` renders them. Outside an Unreal build, the build script reads the lock file; to
see a class you have just named before the next Unreal build, point rust-analyzer at the
exporter's output: `"rust-analyzer.cargo.extraEnv": { "FERREAL_ENGINE_METADATA":
"<Project>/Intermediate/Ferreal/FerrealEngine.json" }`.
The generated C++ sits in the module's `Source`, so the IDE project Unreal generates indexes
it too.

Parameters: the types above, `FName` (as `&str`/`String`), `FRotator`, `FQuat` and
`FTransform` (`Rotator`, `Quat`, `Transform`), the core types below, enums (of every form:
reflection holds an `enum class` as itself and any other as a byte, which the exporter
records), class references, structs, and arrays of these. `FText`, soft references, maps,
sets, delegates or interfaces are left out for now; the generated file lists the functions
they leave out, with why.

**Core types.** `FVector4`, `FIntPoint`, `FIntVector`, `FColor`, `FBox`, `FMatrix`,
`FDateTime` and `FTimespan` are declared to Unreal Header Tool apart from their C++
(NoExport); Ferreal maps them to its own (`Vector4`, `IntPoint`, `IntVector`, `Color`, `Box3`,
`Matrix`, `DateTime`, `Timespan`), copied through their public C++ members (an `FDateTime`
through its ticks).

**Class references.** A `TSubclassOf<C>` (or a `UClass*`, of `UObject`) is a
`ClassOf<C>`: `ClassOf::<engine::Actor>::get()` is `AActor`'s class (found by name), and a
parameter takes the class of anything that `IsA` its class:
`GameplayStatics::get_all_actors_of_class(world, ClassOf::<engine::Actor>::get())`.

**Arrays.** A `TArray` parameter takes a slice (`&[&str]` for strings, `&[Obj<C>]` for
objects), and a returned or output one is a `Vec`. Both are copied element by element: Rust
hands C++ a slice of carriers it builds the `TArray` from, and C++ hands each element of a
`TArray` it returns to Rust, which copies it at once, so arrays of any length (and of strings
or structs) never outlive their storage. Arrays as struct fields are left out for now.

**Outputs.** A reference the function fills (`FHitResult& OutHit`) is returned, after the
return value: `actor.set_actor_location(at, true, false)` returns `(bool, HitResult)`, and
each function's documentation names the values in order. A reference it reads and changes
(`UPARAM(ref) FVector& A`) is a `&mut`: `KismetMathLibrary::vector_normalize(&mut v, 1e-4)`.
Const references are plain inputs.

**Structs.** A struct a bound function passes (`FHitResult`) becomes a Rust struct in the
`engine` module (`engine::HitResult`) holding a copy of its reflected fields: numbers,
strings and names, enums, objects and weak pointers (`Option<Obj<C>>`), the math types, and
structs of the same kind. The copy is made at the boundary, field by field, by generated C++
(`FHitResult` → Rust when returned, Rust → a default `FHitResult` with the fields set when
passed), so no Rust ever points into C++ memory or depends on a struct's layout. A
`const FTransform&` the engine returns is copied the same way, while it is valid. Fields that
are not public, editor-only, deprecated or fixed-size arrays are left out (their struct's
documentation lists them; passed to C++, they keep their defaults), as are core types whose
C++ differs from what Unreal Header Tool is told (NoExport), except the math types Ferreal
maps itself. An `FActorInstanceHandle` (`FHitResult::HitObjectHandle`, whose one field is
private) is copied as its actor, an `Option<Obj<Actor>>`, as `FHitResult::GetActor` returns
it.

**Sharing engine types between crates.** Each crate's `engine` module is its own, so two
Unreal modules' crates that both name `AActor` have two `Actor` types. To pass engine handles,
structs and enums between crates, put the types in a crate of their own, which the modules'
crates depend on:

```rust
// game-types/src/lib.rs (Cargo.toml: a plain library, with `links = "ferreal-types-game"`)
ferreal::engine!(AActor, UPrimitiveComponent);
ferreal::engine_types!();

// game-types/build.rs
fn main() {
    ferreal_build::Types::new().run();
}
```

It holds the markers of the classes it names, and the structs and enums their functions pass,
and no functions or C++. A crate depending on it uses these types as its own (its `engine`
module re-exports them) and binds the functions it calls on them, so an
`Obj<engine::Actor>` or an `engine::HitResult` of one crate is the other's. The static
functions of a shared class are a trait of the crate's, `<Class>Statics`, which `use
engine::*` brings. Cargo hands the crates depending on it what it holds through its `links`
key; each struct is checked to be copied the same way in both (built from the same
reflection data). See [tests/hello/types](tests/hello/types).

## Setting up a module

For a Rust plugin in a project's `Plugins` (as [FerrealSample](examples/UnrealGame/Plugins/FerrealSample)):

1. A Rust crate with `crate-type = ["staticlib"]`, depending on `ferreal` (and on
   `ferreal-build` as a build dependency).
2. Its `build.rs`:
   ```rust
   fn main() {
       ferreal_build::Module::new("MyModule").output("../Source/MyModule").run();
   }
   ```
3. `ferreal::module!();` in its `lib.rs`.
4. The module's `Build.cs`: the "Rust" region of
   [FerrealSample.Build.cs](examples/UnrealGame/Plugins/FerrealSample/Source/FerrealSample/FerrealSample.Build.cs),
   and a call to `BuildRust(...)`. Add a `[profile.unreal]` (release with `panic = "unwind"`)
   to the workspace's `Cargo.toml`, as the example's.
Nothing is installed in the engine, and the plugin depends on no other: the crate's build
writes the module's glue and, beside the module, the exporter (`Source/<Module>Exporter`).
Commit the exporter with the plugin: Unreal Build Tool sets its plugins up before any Build.cs
runs, so the build that first writes it only uses it from the next. With it, every build,
of a fresh checkout too, succeeds the first time.

The generated files can be committed (their diffs show what changed in the C++ API); the
build writes them again either way, and removes those it no longer generates.

## Testing without Unreal

The repository's tasks run on every platform through `cargo xtask` ([xtask](xtask/src/main.rs)):
`test` runs `cargo test`, then the two below; `lint` runs rustfmt and Clippy.

`cargo xtask mock-ue` builds the test plugin, [tests/hello](tests/hello), compiles its C++ (the hand-written parent and the
generated glue) against minimal stand-ins for the engine headers it uses, links the Rust and
runs a test that drives the class through the glue: properties of every type, functions,
overrides, Rust calling C++, change notifications, a caught panic, destruction. It also calls the
engine bindings of its `ferreal::engine!` against a mock engine whose reflection lays
parameters out the way the engine links functions, and whose natives read them through those
offsets: every kind of parameter and return value, static functions, and a function the mock
engine declares differently, which the check must disable. The reflection data it builds
from (`tests/mock-ue/FerrealEngine.json`) is the exporter's own output:
`cargo xtask exporter` runs the exporter's source over a made-up session of Unreal Header
Tool, against a stand-in for UHT's API (it needs the .NET SDK).

These check the glue's C++, its ABI, and the exporter's logic, not Unreal's own tools
(Unreal Header Tool, Unreal Build Tool, UHT's real API), which only a real engine build
exercises.

## Testing in Unreal

`cargo xtask ue [--engine <dir>]` builds [tests/ue-host](tests/ue-host), a project whose
`Plugins` link to the test plugin, with the engine's own tools (the engine: `--engine`, else
`UE_ROOT`, else the launcher's default 5.8). With `--test`, it then runs the host's automation test
([FerrealHostTest.cpp](tests/ue-host/Source/FerrealHost/FerrealHostTest.cpp)) in a headless
editor: the Rust class, and the test plugin's engine calls, structs, outputs, arrays and
`cpp!` blocks, on an actor in a real world. A call Ferreal refuses logs an error, which
fails the test.

## The example game

[examples/UnrealGame](examples/UnrealGame) is Unreal's first-person template with a Rust
plugin, [FerrealSample](examples/UnrealGame/Plugins/FerrealSample): rounds of orbs to collect
([Rust/src/lib.rs](examples/UnrealGame/Plugins/FerrealSample/Rust/src/lib.rs)). The plugin's
module holds two small C++ parents, for what a constructor or the engine's own callbacks must
do; its Build.cs is the template's. The game's own module is
plain C++ and uses the plugin as it would a C++ one: its console commands
([OrbCommands.cpp](examples/UnrealGame/Source/UnrealGame/Private/OrbCommands.cpp)),
`Orbs.Round` and `Orbs.Collect`, include `Ferreal/OrbGame.h` and call the Rust class's
`UFUNCTION`s.

* `AOrb` derives from `AOrbBase` (a sphere that ticks). It overrides `BeginPlay` and `Tick`
  in Rust to circle, bob and take a color of its own. It is `Blueprintable`, with editable
  properties, so you can place orbs in the level too.
* `UOrbGame` derives from `UOrbGameBase`, a world subsystem: the engine creates one in every
  game world, so nothing needs placing or setting. It spawns a ring of orbs around the player
  (in a `cpp!` block, since Blueprint's spawn nodes are internal), collects those the player
  reaches, and shows the score with `PrintString`. Each won round starts a bigger one.

The plugin's crate is a workspace of its own: it builds against the engine it is built in,
not the mock engine the repository's tests use. `cargo xtask game [--engine <dir>]` builds it.
`--play` opens the game in a window, and `cargo xtask editor` opens it in the Unreal Editor.
`--check` plays it headless with `-OrbAutopilot`, which moves the player from orb to orb, and
passes once two rounds are won with no error from Ferreal.

## Limitations

* Built and tested with Unreal 5.8's editor on Win64 (`cargo xtask ue --test`); not yet in a
  packaged game, nor with other engine versions or platforms.
* Engine bindings go through `ProcessEvent`: a lookup and a check per call on top of the
  function, fine for gameplay calls, not for per-element work in a hot loop (write that in a
  `#[cpp_base]` method, or in Rust).
* Two Ferreal modules in one **monolithic** binary each carry the Rust standard library, and
  the linker reports duplicate symbols. Until Ferreal links one shared Rust library per
  binary, put Rust in one module per game (editor builds, one DLL per module, are fine).
* No hot reload or Live Coding of Rust: rebuild the module.
* A `#[uclass]` cannot declare struct (`USTRUCT`), delegate, interface or map properties yet,
  and engine structs are copies, not references: pass their parts, or wrap them in your C++
  base class.
* Consoles need their platform's Rust target, which the platform holders' SDKs do not provide
  publicly (see the Build.cs: add the triple once you have one).
