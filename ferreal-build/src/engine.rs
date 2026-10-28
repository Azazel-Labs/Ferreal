//! Bindings of engine classes, generated on demand from Unreal's reflection data.
//!
//! `ferreal::engine!(AActor, UGameplayStatics)` names the classes a crate calls. Their reflected
//! functions (and their ancestors') become typed Rust: a marker type per class in the crate's
//! `engine` module, an `<Class>Api` trait of its instance functions (implemented by every
//! handle of a class that `IsA` it), its static functions on the marker, and newtypes of the
//! enums they take. Classes the signatures mention get a marker too, so they can be passed
//! and returned; name them in `ferreal::engine!` to call their functions as well.
//!
//! Each call goes through a C++ thunk (in the generated `.cpp`) that fills a struct of the
//! function's parameters and calls `ProcessEvent`, as Unreal's own generated event thunks do.
//! Calling through reflection needs no export from the engine's DLLs, and calls Blueprint
//! overrides of events. The struct is laid out by the C++ compiler; the first call checks
//! it against the function's reflection (its size, and each parameter's name, offset, size
//! and property class) and disables the function, with an error in the log, if they differ.
//!
//! The reflection data comes from the Ferreal plugin's exporter (Unreal Header Tool writes
//! `FerrealEngine.json`), found through `FERREAL_ENGINE_METADATA`. The classes a crate uses are
//! also kept in a lock file next to its `Cargo.toml` (`ferreal-engine.json`): commit it, and a
//! fresh checkout builds before Unreal has exported anything.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// What the exporter writes (`FerrealExporter.cs`).
pub const FORMAT: &str = "ferreal-engine";
pub const FORMAT_VERSION: u32 = 1;
/// The path of the exporter's `FerrealEngine.json` (the Build.cs template sets it).
pub const METADATA_ENV: &str = "FERREAL_ENGINE_METADATA";
/// Printed when a build needs reflection data Unreal has not exported yet: the Build.cs
/// template lets that build go on (Unreal Header Tool then exports it) and asks for another.
pub const NEEDS_METADATA: &str = "FERREAL_NEEDS_ENGINE_METADATA";
/// The lock file's name, next to the crate's `Cargo.toml`.
pub const LOCK_FILE: &str = "ferreal-engine.json";
/// A lock file to use instead of the crate's (for a build from other reflection data, such as
/// a test's, that must not overwrite it).
pub const LOCK_ENV: &str = "FERREAL_ENGINE_LOCK";
/// A module directory to write the C++ into instead of the crate's (with [`LOCK_ENV`]).
pub const OUTPUT_ENV: &str = "FERREAL_OUTPUT";

fn is_false(b: &bool) -> bool {
    !*b
}

/// Unreal's reflection data, as the exporter writes it (and the lock file keeps a part of).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub classes: Vec<ClassMeta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structs: Vec<StructMeta>,
    #[serde(default)]
    pub enums: Vec<EnumMeta>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StructMeta {
    /// `FHitResult`.
    pub cpp: String,
    /// `/Script/Engine.HitResult`.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub module: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(rename = "super", default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Declared to Unreal Header Tool apart from its real C++ (the core math types), whose
    /// fields may not be as declared.
    #[serde(default, skip_serializing_if = "is_false")]
    pub noexport: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(default)]
    pub fields: Vec<FieldMeta>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FieldMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeMeta,
    #[serde(rename = "public", default, skip_serializing_if = "is_false")]
    pub is_public: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub editor_only: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deprecated: bool,
    /// `float Values[4]`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fixed_array: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ClassMeta {
    /// `AActor`.
    pub cpp: String,
    /// `/Script/Engine.Actor`.
    pub path: String,
    #[serde(default)]
    pub module: String,
    /// The header that declares it, as included (`GameFramework/Actor.h`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(rename = "super", default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "abstract", default, skip_serializing_if = "is_false")]
    pub is_abstract: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(default)]
    pub functions: Vec<FunctionMeta>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FunctionMeta {
    /// Its C++ name.
    pub cpp: String,
    /// Its `UFunction`'s name.
    pub name: String,
    #[serde(rename = "static", default, skip_serializing_if = "is_false")]
    pub is_static: bool,
    #[serde(rename = "const", default, skip_serializing_if = "is_false")]
    pub is_const: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub callable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pure: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub event: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub protected: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub net: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub editor_only: bool,
    /// `BlueprintInternalUseOnly`: plumbing of the engine's, never bound.
    #[serde(default, skip_serializing_if = "is_false")]
    pub internal: bool,
    /// Its deprecation message (empty without one), if deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(default)]
    pub params: Vec<ParamMeta>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ParamMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeMeta,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ret: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub out: bool,
    #[serde(rename = "const", default, skip_serializing_if = "is_false")]
    pub is_const: bool,
    #[serde(rename = "ref", default, skip_serializing_if = "is_false")]
    pub is_ref: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TypeMeta {
    /// `bool`, `int32`, `string`, `enum`, `struct`, `object`, ... (`FerrealExporter.cs`).
    pub kind: String,
    /// The engine class of its `FProperty` (`IntProperty`).
    pub property: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(rename = "struct", default, skip_serializing_if = "Option::is_none")]
    pub strukt: Option<String>,
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enumeration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner: Option<Box<TypeMeta>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnumMeta {
    pub cpp: String,
    #[serde(default)]
    pub path: String,
    /// Its underlying type (`uint8`), `unspecified` for an old namespaced enum.
    #[serde(default)]
    pub underlying: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(default)]
    pub values: Vec<EnumValue>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub value: i64,
}

impl Metadata {
    /// Reads and checks a metadata file (the exporter's, or a lock file).
    pub fn load(path: &Path) -> Result<Metadata, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let m: Metadata =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if m.format != FORMAT || m.version != FORMAT_VERSION {
            return Err(format!(
                "{}: not Ferreal's reflection data of version {FORMAT_VERSION} (format {:?}, version {}); update the Ferreal plugin and ferreal-build together",
                path.display(),
                m.format,
                m.version
            ));
        }
        Ok(m)
    }
}

/// A class named in `ferreal::engine!` (its C++ name), and where.
#[derive(Clone, Debug)]
pub struct Request {
    pub cpp: String,
    pub at: String,
}

/// A Rust type that is also a class: a `#[uclass]` struct (`own`) or an `engine_class!`
/// marker, by its path from the crate root and the C++ class it is. The bindings use it as
/// that class' marker (and implement `IsA` of its ancestors for it); the functions of a
/// `#[uclass]` are the crate's own, and are not bound.
#[derive(Clone, Debug)]
pub struct Handle {
    pub rust_path: String,
    pub cpp: String,
    pub own: bool,
}

/// How the bindings name a class.
#[derive(Clone, Debug)]
struct Marker {
    /// The type, as the generated code spells it (`Actor`, `crate::Spinner`).
    path: String,
    /// Its bare name (`Actor`), which names its `<Name>Api` trait.
    name: String,
    /// Declared by the generated code (else by the crate: a `Handle`).
    generated: bool,
}

/// A parameter type the bindings pass.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PTy {
    Bool,
    I32,
    I64,
    U8,
    U32,
    F32,
    F64,
    Str,
    Name,
    /// An enum, by its C++ name: its values are `value`s (its Rust newtype's), held in the
    /// parameter as `member`s (a byte for a TEnumAsByte, else the enum's own type).
    Enum {
        name: String,
        value: Int,
        member: Int,
    },
    Vector,
    Vector2D,
    LinearColor,
    Rotator,
    Quat,
    Transform,
    /// An object of this C++ class.
    Obj(String),
    /// A struct, by its C++ name, copied field by field (see [`StructPlan`]).
    Struct(String),
    /// A weak pointer to an object of this C++ class (struct fields only).
    Weak(String),
}

/// How a struct is copied: the fields Ferreal carries, and those it leaves out (at their C++
/// defaults when Rust passes the struct to C++).
#[derive(Clone, Debug)]
struct StructPlan {
    meta: StructMeta,
    fields: Vec<FieldPlan>,
    left_out: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
struct FieldPlan {
    rust: String,
    cpp: String,
    ty: PTy,
    doc: Option<String>,
}

/// The reflection data the bindings are resolved against, and the structs planned so far.
struct Types<'m> {
    classes: &'m HashMap<&'m str, &'m ClassMeta>,
    enums: &'m HashMap<&'m str, &'m EnumMeta>,
    structs: HashMap<&'m str, &'m StructMeta>,
    plans: BTreeMap<String, Result<StructPlan, String>>,
    /// The structs looked at, which the lock file keeps.
    consulted: BTreeSet<String>,
}

impl Types<'_> {
    fn param_type(&mut self, t: &TypeMeta) -> Result<PTy, String> {
        Ok(match t.kind.as_str() {
            "bool" => PTy::Bool,
            "int32" => PTy::I32,
            "int64" => PTy::I64,
            "uint8" => PTy::U8,
            "uint32" => PTy::U32,
            "float" => PTy::F32,
            "double" => PTy::F64,
            "string" => PTy::Str,
            "name" => PTy::Name,
            "enum" => {
                let name = t.enumeration.clone().unwrap_or_default();
                let Some(e) = self.enums.get(name.as_str()) else {
                    return Err(format!("enum {name}, which was not exported"));
                };
                let value = enum_value_type(e);
                // A TEnumAsByte (a ByteProperty) is a byte whatever the enum's type; an
                // EnumProperty holds the enum's own type, which an old namespaced enum leaves
                // to the compiler.
                let member = if t.property == "ByteProperty" {
                    Int::U8
                } else {
                    Int::of_enum(&e.underlying)
                        .ok_or_else(|| format!("enum {name}, of {} values", e.underlying))?
                };
                PTy::Enum {
                    name,
                    value,
                    member,
                }
            }
            "struct" => self.struct_type(t.strukt.as_deref().unwrap_or("?"))?,
            "object" => {
                let c = t.class.clone().unwrap_or_else(|| "UObject".into());
                if !self.classes.contains_key(c.as_str()) {
                    return Err(format!("an object of {c}, which was not exported"));
                }
                PTy::Obj(c)
            }
            k => return Err(format!("a {k} ({})", t.property)),
        })
    }

    /// A struct's type: Ferreal's own for the math types it maps, else a copy of its fields.
    fn struct_type(&mut self, cpp: &str) -> Result<PTy, String> {
        match cpp {
            "FVector" => return Ok(PTy::Vector),
            "FVector2D" => return Ok(PTy::Vector2D),
            "FLinearColor" => return Ok(PTy::LinearColor),
            "FRotator" => return Ok(PTy::Rotator),
            "FQuat" => return Ok(PTy::Quat),
            "FTransform" => return Ok(PTy::Transform),
            _ => {}
        }
        let Some(meta) = self.structs.get(cpp).copied() else {
            return Err(format!("struct {cpp}, which was not exported"));
        };
        self.consulted.insert(cpp.to_string());
        // A struct adding no field to a mapped one (`FVector_NetQuantize`) is that one.
        if meta.fields.is_empty()
            && let Some(parent) = &meta.parent
            && let Ok(t) = self.struct_type(parent)
            && t.is_math()
        {
            return Ok(t);
        }
        if meta.noexport {
            return Err(format!(
                "struct {cpp}, a core type declared to Unreal Header Tool apart from its C++"
            ));
        }
        self.plan(cpp)?;
        Ok(PTy::Struct(cpp.to_string()))
    }

    fn plan(&mut self, cpp: &str) -> Result<(), String> {
        if let Some(p) = self.plans.get(cpp) {
            return p.as_ref().map(|_| ()).map_err(Clone::clone);
        }
        self.plans
            .insert(cpp.into(), Err(format!("struct {cpp}, which holds itself")));
        let meta = self.structs[cpp];
        // Its ancestors' fields first.
        let mut chain = vec![meta];
        let mut at = meta.parent.clone();
        while let Some(p) = at {
            match self.structs.get(p.as_str()).copied() {
                Some(m) if !m.noexport && chain.len() < 16 => {
                    self.consulted.insert(p.clone());
                    chain.push(m);
                    at = m.parent.clone();
                }
                _ => break,
            }
        }
        chain.reverse();
        let mut fields = Vec::new();
        let mut left_out = Vec::new();
        let mut names = BTreeSet::new();
        for m in chain {
            for f in &m.fields {
                let why = if !f.is_public {
                    Some("not public")
                } else if f.editor_only {
                    Some("editor only")
                } else if f.deprecated {
                    Some("deprecated")
                } else if f.fixed_array {
                    Some("an array of a fixed size")
                } else {
                    None
                };
                let ty = match why {
                    Some(w) => Err(w.to_string()),
                    None if f.ty.kind == "weak" => {
                        let c = f.ty.class.clone().unwrap_or_else(|| "UObject".into());
                        if self.classes.contains_key(c.as_str()) {
                            Ok(PTy::Weak(c))
                        } else {
                            Err(format!("a weak pointer to {c}, which was not exported"))
                        }
                    }
                    None => self.param_type(&f.ty),
                };
                match ty {
                    Ok(ty) => {
                        let mut rust = param_name(&f.name, ty == PTy::Bool);
                        while !names.insert(rust.clone()) {
                            rust.push('_');
                        }
                        fields.push(FieldPlan {
                            rust,
                            cpp: f.name.clone(),
                            ty,
                            doc: f.doc.clone(),
                        });
                    }
                    Err(why) => left_out.push((f.name.clone(), why)),
                }
            }
        }
        let plan = if fields.is_empty() {
            Err(format!("struct {cpp}, none of whose fields Ferreal copies"))
        } else {
            Ok(StructPlan {
                meta: meta.clone(),
                fields,
                left_out,
            })
        };
        let r = plan.as_ref().map(|_| ()).map_err(Clone::clone);
        self.plans.insert(cpp.into(), plan);
        r
    }
}

/// An enum's integer type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Int {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
}

impl Int {
    /// An enum's underlying type as the exporter writes it; None for `unspecified`.
    fn of_enum(underlying: &str) -> Option<Int> {
        Some(match underlying {
            "uint8" => Int::U8,
            "int8" => Int::I8,
            "uint16" => Int::U16,
            "int16" => Int::I16,
            "uint32" => Int::U32,
            "int32" => Int::I32,
            "uint64" => Int::U64,
            "int64" => Int::I64,
            _ => return None,
        })
    }

    fn rust(self) -> &'static str {
        match self {
            Int::U8 => "u8",
            Int::I8 => "i8",
            Int::U16 => "u16",
            Int::I16 => "i16",
            Int::U32 => "u32",
            Int::I32 => "i32",
            Int::U64 => "u64",
            Int::I64 => "i64",
        }
    }

    fn cpp(self) -> &'static str {
        match self {
            Int::U8 => "uint8",
            Int::I8 => "int8",
            Int::U16 => "uint16",
            Int::I16 => "int16",
            Int::U32 => "uint32",
            Int::I32 => "int32",
            Int::U64 => "uint64",
            Int::I64 => "int64",
        }
    }

    fn holds(self, v: i64) -> bool {
        match self {
            Int::U8 => u8::try_from(v).is_ok(),
            Int::I8 => i8::try_from(v).is_ok(),
            Int::U16 => u16::try_from(v).is_ok(),
            Int::I16 => i16::try_from(v).is_ok(),
            Int::U32 => u32::try_from(v).is_ok(),
            Int::I32 => i32::try_from(v).is_ok(),
            Int::U64 => v >= 0,
            Int::I64 => true,
        }
    }
}

/// The type of an enum's values in Rust: its own, or a byte for an old namespaced enum (only
/// passed as a TEnumAsByte).
fn enum_value_type(e: &EnumMeta) -> Int {
    Int::of_enum(&e.underlying).unwrap_or(Int::U8)
}

#[derive(Clone, Debug)]
struct BoundParam {
    rust: String,
    cpp: String,
    ty: PTy,
    property: String,
    strukt: Option<String>,
}

#[derive(Clone, Debug)]
struct Bound {
    class: String,
    function: FunctionMeta,
    rust: String,
    params: Vec<BoundParam>,
    ret: Option<BoundParam>,
}

/// The bindings of a crate: what goes into its `engine` module, its `.cpp` and its lock file.
#[derive(Clone, Debug, Default)]
pub struct Bindings {
    /// The markers of every class involved, by C++ name.
    markers: BTreeMap<String, Marker>,
    /// The classes, in order from the roots (`UObject` first).
    classes: Vec<ClassMeta>,
    /// The classes whose functions are bound.
    bound_classes: BTreeSet<String>,
    functions: Vec<Bound>,
    enums: Vec<EnumMeta>,
    /// The structs passed, those their fields hold first.
    structs: Vec<StructPlan>,
    /// Their Rust names, by C++ name.
    struct_names: BTreeMap<String, String>,
    /// Functions left out, with why (shown in the generated file).
    skipped: Vec<(String, String)>,
    /// What the lock file keeps.
    pub lock: Metadata,
}

/// `Rotator`, `Quat`, `Transform`: math types `cpp!` passes by name, Ferreal's own.
fn is_math(name: &str) -> bool {
    matches!(name, "Rotator" | "Quat" | "Transform")
}

/// A type a `cpp!` block passes by name, as its C++ sees it.
pub struct Named(PTy);

impl Named {
    /// Its C++ value (an engine enum as its integer).
    pub fn cpp_value(&self) -> String {
        match &self.0 {
            PTy::Enum { value, .. } => value.cpp().into(),
            t => t.cpp_member(),
        }
    }

    /// Its carrier from Rust (`inbound` false) or to Rust; `scope` qualifies the generated
    /// structs' carriers and conversions (`Ferreal::<Module>::Detail::`).
    pub fn cpp_carrier(&self, inbound: bool, scope: &str) -> String {
        match &self.0 {
            PTy::Struct(_) => format!("{scope}{}", self.0.cpp_carrier(inbound)),
            t => t.cpp_carrier(inbound),
        }
    }

    pub fn from_carrier(&self, e: &str, scope: &str) -> String {
        match &self.0 {
            PTy::Struct(s) => format!("{scope}FromRust_{s}({e})"),
            PTy::Enum { .. } => e.into(),
            t => t.cpp_from_carrier(e),
        }
    }

    pub fn to_carrier(&self, e: &str, scope: &str) -> String {
        match &self.0 {
            PTy::Struct(s) => format!("{scope}ToRust_{s}({e})"),
            PTy::Enum { .. } => e.into(),
            t => t.cpp_to_carrier(e),
        }
    }
}

impl Bindings {
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// A type `cpp!` passes by its Rust name: a math type, a generated struct or enum.
    pub fn named(&self, name: &str) -> Option<Named> {
        match name {
            "Rotator" => return Some(Named(PTy::Rotator)),
            "Quat" => return Some(Named(PTy::Quat)),
            "Transform" => return Some(Named(PTy::Transform)),
            _ => {}
        }
        if let Some((cpp, _)) = self.struct_names.iter().find(|(_, n)| *n == name) {
            return Some(Named(PTy::Struct(cpp.clone())));
        }
        self.enums
            .iter()
            .find(|e| enum_rust_name(self, &e.cpp) == name)
            .map(|e| {
                let value = enum_value_type(e);
                Named(PTy::Enum {
                    name: e.cpp.clone(),
                    value,
                    member: value,
                })
            })
    }

    /// The C++ of the structs copied: their carriers and conversions (in
    /// `Ferreal::<M>::Detail`).
    pub fn cpp_structs(&self) -> String {
        let mut o = String::new();
        if self.structs.is_empty() {
            return o;
        }
        // Fields C++ deprecates without telling reflection are copied too.
        o.push_str("\nPRAGMA_DISABLE_DEPRECATION_WARNINGS\n");
        for p in &self.structs {
            self.cpp_struct(&mut o, p);
        }
        o.push_str("PRAGMA_ENABLE_DEPRECATION_WARNINGS\n");
        o
    }

    /// The marker of a C++ class (`AActor` → `Actor`), if the bindings have one.
    pub fn marker(&self, cpp: &str) -> Option<&str> {
        self.markers.get(cpp).map(|m| m.path.as_str())
    }

    /// The generated markers (by Rust name) with their C++ class and header, for
    /// `Obj<engine::X>` in `#[uclass]` items.
    pub fn marker_classes(&self) -> impl Iterator<Item = (&str, &str, Option<&str>)> {
        self.classes.iter().filter_map(|c| {
            let m = &self.markers[&c.cpp];
            m.generated
                .then_some((m.name.as_str(), c.cpp.as_str(), c.header.as_deref()))
        })
    }

    fn path(&self, cpp: &str) -> &str {
        &self.markers[cpp].path
    }
}

/// Where the bindings come from: the exporter's file, else the lock file.
pub enum Source {
    Exported(Metadata),
    Lock(Metadata),
    None,
}

/// Resolves `requests` (and the crate's own `handles`) against the metadata.
///
/// Errors carry [`NEEDS_METADATA`] when a later build can fix them (Unreal has not exported
/// the data yet, or the lock file lacks a class newly named).
///
/// `named` are the types `cpp!` blocks pass by name (`HitResult`): engine structs (`F...`) and
/// enums (`E...`), generated whether or not a bound function passes them.
pub fn resolve(
    source: &Source,
    requests: &[Request],
    handles: &[Handle],
    named: &[Request],
) -> Result<Bindings, Vec<String>> {
    let named: Vec<&Request> = named.iter().filter(|n| !is_math(&n.cpp)).collect();
    let (meta, exported) = match source {
        Source::Exported(m) => (m, true),
        Source::Lock(m) => (m, false),
        Source::None => {
            let Some(first) = requests.first().or(named.first().copied()) else {
                return Ok(Bindings::default());
            };
            return Err(vec![format!(
                "{}: the crate uses engine classes or types, but there is no reflection data yet. Enable the Ferreal plugin and build the Unreal project: Unreal Header Tool exports the data during that build, and the next build generates the bindings ({NEEDS_METADATA}). Outside Unreal builds, commit the {} that build writes, or set {METADATA_ENV} to the exporter's FerrealEngine.json.",
                first.at, LOCK_FILE
            )]);
        }
    };
    let classes: HashMap<&str, &ClassMeta> =
        meta.classes.iter().map(|c| (c.cpp.as_str(), c)).collect();
    let enums: HashMap<&str, &EnumMeta> = meta.enums.iter().map(|e| (e.cpp.as_str(), e)).collect();
    let mut types = Types {
        classes: &classes,
        enums: &enums,
        structs: meta.structs.iter().map(|s| (s.cpp.as_str(), s)).collect(),
        plans: BTreeMap::new(),
        consulted: BTreeSet::new(),
    };
    let ancestors = |cpp: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut at = Some(cpp.to_string());
        while let Some(c) = at {
            match classes.get(c.as_str()) {
                Some(m) => {
                    at = m.parent.clone();
                    out.push(c);
                }
                None => break,
            }
            if out.len() > 64 {
                break;
            }
        }
        out
    };

    let mut errors = Vec::new();
    let mut bound = BTreeSet::new();
    for r in requests {
        if classes.contains_key(r.cpp.as_str()) {
            bound.extend(ancestors(&r.cpp));
        } else if exported {
            let near: Vec<&str> = classes
                .keys()
                .filter(|k| k.to_lowercase().contains(&r.cpp[1..].to_lowercase()))
                .take(5)
                .copied()
                .collect();
            errors.push(format!(
                "{}: {} is not a reflected class of this project's engine{}",
                r.at,
                r.cpp,
                if near.is_empty() {
                    String::new()
                } else {
                    format!(" (did you mean {}?)", near.join(", "))
                }
            ));
        } else {
            errors.push(format!(
                "{}: {} is not in {LOCK_FILE} yet: build the Unreal project with the Ferreal plugin enabled to export it ({NEEDS_METADATA})",
                r.at, r.cpp
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // The functions of the bound classes, and the classes and enums their signatures need.
    let mut needed: BTreeSet<String> = bound.clone();
    let mut used_enums = BTreeSet::new();
    let mut functions = Vec::new();
    let mut skipped = Vec::new();
    let own: BTreeSet<&str> = handles
        .iter()
        .filter(|h| h.own)
        .map(|h| h.cpp.as_str())
        .collect();
    for cpp in &bound {
        let class = classes[cpp.as_str()];
        if own.contains(cpp.as_str()) {
            continue;
        }
        let mut names = BTreeSet::new();
        for f in &class.functions {
            if f.internal {
                continue;
            }
            match bind(class, f, &mut types) {
                Ok(b) => {
                    for p in b.params.iter().chain(&b.ret) {
                        match &p.ty {
                            PTy::Obj(c) => needed.extend(ancestors(c)),
                            PTy::Enum { name, .. } => {
                                used_enums.insert(name.clone());
                            }
                            _ => {}
                        }
                    }
                    names.insert(b.rust.clone());
                    functions.push(b);
                }
                Err(why) => skipped.push((format!("{}::{}", class.cpp, f.cpp), why)),
            }
        }
        // `K2_` functions lose the prefix, unless another function has that name.
        for b in functions.iter_mut().filter(|b| &b.class == cpp) {
            if let Some(bare) = b.rust.strip_prefix("k2_")
                && !names.contains(bare)
            {
                b.rust = bare.to_string();
            }
        }
    }
    for h in handles {
        if classes.contains_key(h.cpp.as_str()) {
            needed.extend(ancestors(&h.cpp));
        }
    }

    // The types `cpp!` blocks pass by name.
    let mut named_structs = Vec::new();
    for n in &named {
        let (strukt, enumeration) = (format!("F{}", n.cpp), format!("E{}", n.cpp));
        if types.structs.contains_key(strukt.as_str()) {
            match types.struct_type(&strukt) {
                Ok(PTy::Struct(s)) => named_structs.push(s),
                Ok(_) => {}
                Err(why) => errors.push(format!("{}: {}: Ferreal cannot copy {why}", n.at, n.cpp)),
            }
        } else if enums.contains_key(enumeration.as_str()) {
            used_enums.insert(enumeration);
        } else if exported {
            errors.push(format!(
                "{}: {} is not a #[uenum], an engine struct ({strukt}) or an engine enum ({enumeration})",
                n.at, n.cpp
            ));
        } else {
            errors.push(format!(
                "{}: {} is not in {LOCK_FILE} yet: build the Unreal project with the Ferreal plugin enabled to export it ({NEEDS_METADATA})",
                n.at, n.cpp
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // The structs the functions pass, those their fields hold first; the classes and enums
    // their fields name.
    let mut structs = Vec::new();
    let mut seen = BTreeSet::new();
    for p in functions
        .iter()
        .flat_map(|b: &Bound| b.params.iter().chain(&b.ret))
    {
        if let PTy::Struct(s) = &p.ty {
            order_structs(&types.plans, s, &mut seen, &mut structs);
        }
    }
    for s in &named_structs {
        order_structs(&types.plans, s, &mut seen, &mut structs);
    }
    for p in &structs {
        for f in &p.fields {
            match &f.ty {
                PTy::Obj(c) | PTy::Weak(c) => needed.extend(ancestors(c)),
                PTy::Enum { name, .. } => {
                    used_enums.insert(name.clone());
                }
                _ => {}
            }
        }
    }

    // Markers: the crate's own types where it has them, else generated, named by the C++
    // name without its prefix (unless two classes would share it).
    let mut markers = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for cpp in &needed {
        let bare = cpp.get(1..).unwrap_or(cpp).to_string();
        let name = if taken.insert(bare.clone()) {
            bare
        } else {
            cpp.clone()
        };
        let marker = match handles.iter().find(|h| &h.cpp == cpp) {
            Some(h) => Marker {
                path: h.rust_path.clone(),
                name,
                generated: false,
            },
            None => Marker {
                path: name.clone(),
                name,
                generated: true,
            },
        };
        markers.insert(cpp.clone(), marker);
    }
    // Structs by their C++ name without the `F`, unless a class marker has that name.
    let struct_names: BTreeMap<String, String> = structs
        .iter()
        .map(|p| {
            let cpp = &p.meta.cpp;
            let bare = cpp
                .strip_prefix('F')
                .filter(|r| r.starts_with(|c: char| c.is_uppercase()))
                .unwrap_or(cpp);
            let name = if markers.values().any(|m: &Marker| m.name == bare) {
                format!("{bare}Struct")
            } else {
                bare.to_string()
            };
            (cpp.clone(), name)
        })
        .collect();
    let mut enum_list: Vec<EnumMeta> = used_enums
        .iter()
        .filter_map(|e| enums.get(e.as_str()).map(|m| (*m).clone()))
        .collect();
    enum_list.sort_by(|a, b| a.cpp.cmp(&b.cpp));

    // Classes in order from the roots, so a reader meets parents first.
    let mut ordered: Vec<ClassMeta> = needed
        .iter()
        .map(|c| (*classes[c.as_str()]).clone())
        .collect();
    ordered.sort_by_key(|c| (ancestors(&c.cpp).len(), c.cpp.clone()));

    let lock = Metadata {
        format: FORMAT.into(),
        version: FORMAT_VERSION,
        classes: ordered
            .iter()
            .map(|c| {
                let mut c = c.clone();
                if !bound.contains(&c.cpp) {
                    c.functions.clear();
                }
                c
            })
            .collect(),
        structs: types
            .consulted
            .iter()
            .filter_map(|s| types.structs.get(s.as_str()).map(|m| (*m).clone()))
            .collect(),
        enums: enum_list.clone(),
    };
    for c in &mut ordered {
        if !bound.contains(&c.cpp) {
            c.functions.clear();
        }
    }
    Ok(Bindings {
        markers,
        classes: ordered,
        functions,
        enums: enum_list,
        structs,
        struct_names,
        bound_classes: bound
            .into_iter()
            .filter(|c| !own.contains(c.as_str()))
            .collect(),
        skipped,
        lock,
    })
}

/// Adds struct `s` (a planned one) to `out`, after the structs its fields hold.
fn order_structs(
    plans: &BTreeMap<String, Result<StructPlan, String>>,
    s: &str,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<StructPlan>,
) {
    if !seen.insert(s.to_string()) {
        return;
    }
    let Some(Ok(p)) = plans.get(s) else {
        return;
    };
    for f in &p.fields {
        if let PTy::Struct(n) = &f.ty {
            order_structs(plans, n, seen, out);
        }
    }
    out.push(p.clone());
}

/// How a function binds, or why it does not.
fn bind(class: &ClassMeta, f: &FunctionMeta, types: &mut Types<'_>) -> Result<Bound, String> {
    let mut params = Vec::new();
    let mut ret = None;
    let mut used = BTreeSet::new();
    for p in &f.params {
        if p.out && !p.is_const {
            return Err(format!("its output parameter {}", p.name));
        }
        let ty = types
            .param_type(&p.ty)
            .map_err(|t| format!("{} is {t}", p.name))?;
        let mut rust = param_name(&p.name, ty == PTy::Bool);
        while !used.insert(rust.clone()) {
            rust.push('_');
        }
        let bp = BoundParam {
            rust,
            cpp: p.name.clone(),
            property: property_classes(&p.ty),
            strukt: (p.ty.kind == "struct")
                .then(|| {
                    p.ty.strukt
                        .as_ref()
                        .map(|s| s.get(1..).unwrap_or(s).to_string())
                })
                .flatten(),
            ty,
        };
        if p.ret {
            ret = Some(bp);
        } else {
            params.push(bp);
        }
    }
    Ok(Bound {
        class: class.cpp.clone(),
        function: f.clone(),
        rust: method_name(&f.cpp),
        params,
        ret,
    })
}

/// The property classes the generated check accepts for a parameter.
fn property_classes(t: &TypeMeta) -> String {
    match t.property.as_str() {
        "DoubleProperty" | "LargeWorldCoordinatesRealProperty" => {
            "DoubleProperty|LargeWorldCoordinatesRealProperty".into()
        }
        p => p.into(),
    }
}

/// `K2_SetActorLocation` → `k2_set_actor_location`, `GetHUD` → `get_hud`.
pub fn snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
            continue;
        }
        if c.is_uppercase() && i > 0 && !out.ends_with('_') {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev.is_lowercase() || (prev.is_uppercase() && next_lower) {
                out.push('_');
            }
        }
        out.extend(c.to_lowercase());
    }
    out.trim_end_matches('_').to_string()
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while",
    "abstract", "become", "box", "do", "final", "gen", "macro", "override", "priv", "try",
    "typeof", "unsized", "virtual", "yield",
];

fn ident(name: String) -> String {
    match name.as_str() {
        "self" | "super" | "crate" | "Self" | "" => format!("{name}_"),
        n if KEYWORDS.contains(&n) => format!("r#{n}"),
        n if n.starts_with(|c: char| c.is_ascii_digit()) => format!("_{n}"),
        _ => name,
    }
}

fn method_name(cpp: &str) -> String {
    ident(snake(cpp))
}

/// `bNewHidden` → `new_hidden` (booleans lose their `b`).
fn param_name(cpp: &str, boolean: bool) -> String {
    let bare = match cpp.strip_prefix('b') {
        Some(rest) if boolean && rest.starts_with(|c: char| c.is_uppercase()) => rest,
        _ => cpp,
    };
    ident(snake(bare))
}

/// `NoCollision` → `NO_COLLISION`.
fn constant_name(value: &str) -> String {
    ident(snake(value).to_uppercase())
}

/// A doc comment of `text`, each line as an attribute (trimmed, so no line is taken for an
/// indented code block).
fn doc(out: &mut String, indent: &str, text: &str) {
    for line in text.lines() {
        // No code fences: rustdoc would test what they hold.
        let mut line = line.trim().replace("```", "'''");
        if line.starts_with("~~~") {
            line.insert(0, '\\');
        }
        let _ = writeln!(out, "{indent}#[doc = {:?}]", line);
    }
}

impl PTy {
    /// One of the math types Ferreal maps to its own (`FVector` → `Vector3`).
    fn is_math(&self) -> bool {
        matches!(
            self,
            PTy::Vector
                | PTy::Vector2D
                | PTy::LinearColor
                | PTy::Rotator
                | PTy::Quat
                | PTy::Transform
        )
    }

    /// The Rust type of a parameter (`generic`: the type parameter of an object's class).
    fn rust_param(&self, b: &Bindings, generic: &str) -> String {
        match self {
            PTy::Str | PTy::Name => "&str".into(),
            PTy::Obj(_) => format!("::core::option::Option<::ferreal::Obj<{generic}>>"),
            PTy::Struct(s) => format!("&{}", b.struct_names[s]),
            _ => self.rust_value(b),
        }
    }

    /// The Rust type of a returned value.
    fn rust_value(&self, b: &Bindings) -> String {
        match self {
            PTy::Bool => "bool".into(),
            PTy::I32 => "i32".into(),
            PTy::I64 => "i64".into(),
            PTy::U8 => "u8".into(),
            PTy::U32 => "u32".into(),
            PTy::F32 => "f32".into(),
            PTy::F64 => "f64".into(),
            PTy::Str | PTy::Name => "::std::string::String".into(),
            PTy::Enum { name, .. } => enum_rust_name(b, name),
            PTy::Vector => "::ferreal::Vector3".into(),
            PTy::Vector2D => "::ferreal::Vector2".into(),
            PTy::LinearColor => "::ferreal::LinearColor".into(),
            PTy::Rotator => "::ferreal::Rotator".into(),
            PTy::Quat => "::ferreal::Quat".into(),
            PTy::Transform => "::ferreal::Transform".into(),
            PTy::Obj(c) | PTy::Weak(c) => {
                format!("::core::option::Option<::ferreal::Obj<{}>>", b.path(c))
            }
            PTy::Struct(s) => b.struct_names[s].clone(),
        }
    }

    /// The carrier Rust passes to C++.
    fn rust_carrier_out(&self) -> String {
        match self {
            PTy::Str | PTy::Name => "::ferreal::ffi::Str8".into(),
            PTy::Enum { value, .. } => value.rust().into(),
            PTy::Obj(_) | PTy::Weak(_) => "::ferreal::ffi::RawObj".into(),
            PTy::Struct(s) => format!("{s}Out"),
            _ => self.rust_value_plain(),
        }
    }

    /// The carrier C++ returns to Rust.
    fn rust_carrier_in(&self) -> String {
        match self {
            PTy::Str | PTy::Name => "::ferreal::ffi::Str16".into(),
            PTy::Enum { value, .. } => value.rust().into(),
            PTy::Obj(_) | PTy::Weak(_) => "::ferreal::ffi::RawObj".into(),
            PTy::Struct(s) => format!("{s}In"),
            _ => self.rust_value_plain(),
        }
    }

    fn rust_value_plain(&self) -> String {
        match self {
            PTy::Bool => "bool",
            PTy::I32 => "i32",
            PTy::I64 => "i64",
            PTy::U8 => "u8",
            PTy::U32 => "u32",
            PTy::F32 => "f32",
            PTy::F64 => "f64",
            PTy::Vector => "::ferreal::Vector3",
            PTy::Vector2D => "::ferreal::Vector2",
            PTy::LinearColor => "::ferreal::LinearColor",
            PTy::Rotator => "::ferreal::Rotator",
            PTy::Quat => "::ferreal::Quat",
            PTy::Transform => "::ferreal::Transform",
            _ => unreachable!("not a plain carrier"),
        }
        .into()
    }

    /// Rust argument `v` as its carrier (`v` a reference for strings and structs).
    fn rust_to_carrier(&self, v: &str) -> String {
        match self {
            PTy::Str | PTy::Name => format!("::ferreal::ffi::Str8::new({v})"),
            PTy::Enum { .. } => format!("{v}.0"),
            PTy::Obj(_) | PTy::Weak(_) => {
                format!("{v}.map_or(::ferreal::ffi::RawObj::NULL, |o| o.raw())")
            }
            PTy::Struct(_) => format!("::ferreal::Carry::to_out({v})"),
            _ => v.into(),
        }
    }

    /// Carrier `v` returned by C++ as the Rust value.
    fn rust_from_carrier(&self, b: &Bindings, v: &str) -> String {
        match self {
            PTy::Str | PTy::Name => format!("{v}.to_string_lossy()"),
            PTy::Enum { name, .. } => format!("{}({v})", enum_rust_name(b, name)),
            PTy::Obj(_) | PTy::Weak(_) => format!("::ferreal::Obj::from_raw({v})"),
            PTy::Struct(s) => format!("<{} as ::ferreal::Carry>::from_in({v})", b.struct_names[s]),
            _ => v.into(),
        }
    }

    /// The type of the parameter's member in the C++ struct of parameters.
    fn cpp_member(&self) -> String {
        match self {
            PTy::Bool => "bool",
            PTy::I32 => "int32",
            PTy::I64 => "int64",
            PTy::U8 => "uint8",
            PTy::Enum { member, .. } => member.cpp(),
            PTy::U32 => "uint32",
            PTy::F32 => "float",
            PTy::F64 => "double",
            PTy::Str => "FString",
            PTy::Name => "FName",
            PTy::Vector => "FVector",
            PTy::Vector2D => "FVector2D",
            PTy::LinearColor => "FLinearColor",
            PTy::Rotator => "FRotator",
            PTy::Quat => "FQuat",
            PTy::Transform => "FTransform",
            PTy::Obj(_) => "UObject*",
            PTy::Struct(s) => s,
            PTy::Weak(c) => return format!("TWeakObjectPtr<{c}>"),
        }
        .into()
    }

    /// The C++ carrier from Rust (`inbound` false) or to Rust (`inbound` true).
    fn cpp_carrier(&self, inbound: bool) -> String {
        match self {
            PTy::Str | PTy::Name if inbound => "Ferreal::FStr16".into(),
            PTy::Str | PTy::Name => "Ferreal::FStr8".into(),
            PTy::Vector => "Ferreal::FVec3".into(),
            PTy::Vector2D => "Ferreal::FVec2".into(),
            PTy::LinearColor => "Ferreal::FColor4".into(),
            PTy::Rotator => "Ferreal::FRot".into(),
            PTy::Quat => "Ferreal::FQuat4".into(),
            PTy::Transform => "Ferreal::FXform".into(),
            PTy::Obj(_) | PTy::Weak(_) => "void*".into(),
            PTy::Enum { value, .. } => value.cpp().into(),
            PTy::Struct(s) => format!("FStruct_{s}_{}", if inbound { "In" } else { "Out" }),
            t => t.cpp_member(),
        }
    }

    /// C++ carrier `e` from Rust as the member's value.
    fn cpp_from_carrier(&self, e: &str) -> String {
        match self {
            PTy::Str => format!("Ferreal::ToFString({e})"),
            PTy::Name => format!("FName(*Ferreal::ToFString({e}))"),
            PTy::Vector => format!("Ferreal::FromVec3({e})"),
            PTy::Vector2D => format!("Ferreal::FromVec2({e})"),
            PTy::LinearColor => format!("Ferreal::FromColor4({e})"),
            PTy::Rotator => format!("Ferreal::FromRot({e})"),
            PTy::Quat => format!("Ferreal::FromQuat({e})"),
            PTy::Transform => format!("Ferreal::FromXform({e})"),
            PTy::Obj(_) => format!("static_cast<UObject*>({e})"),
            PTy::Weak(c) => format!("Ferreal::FromObj<{c}>({e})"),
            PTy::Enum { member, .. } => format!("static_cast<{}>({e})", member.cpp()),
            PTy::Struct(s) => format!("FromRust_{s}({e})"),
            _ => e.into(),
        }
    }

    /// Member `e` as the carrier returned to Rust (kept past the thunk).
    fn cpp_to_carrier(&self, e: &str) -> String {
        match self {
            PTy::Str => format!("Ferreal::KeepString({e})"),
            PTy::Name => format!("Ferreal::KeepString({e}.ToString())"),
            PTy::Vector => format!("Ferreal::ToVec3({e})"),
            PTy::Vector2D => format!("Ferreal::ToVec2({e})"),
            PTy::LinearColor => format!("Ferreal::ToColor4({e})"),
            PTy::Rotator => format!("Ferreal::ToRot({e})"),
            PTy::Quat => format!("Ferreal::ToQuat({e})"),
            PTy::Transform => format!("Ferreal::ToXform({e})"),
            PTy::Obj(_) => format!("Ferreal::ToObj({e})"),
            PTy::Weak(_) => format!("Ferreal::ToObj({e}.Get())"),
            PTy::Enum { value, .. } => format!("static_cast<{}>({e})", value.cpp()),
            PTy::Struct(s) => format!("ToRust_{s}({e})"),
            _ => e.into(),
        }
    }
}

fn enum_rust_name(b: &Bindings, cpp: &str) -> String {
    let bare = cpp
        .strip_prefix('E')
        .filter(|r| r.starts_with(|c: char| c.is_uppercase()))
        .unwrap_or(cpp);
    if b.markers.values().any(|m| m.name == bare) {
        format!("{bare}Enum")
    } else {
        bare.to_string()
    }
}

impl Bindings {
    /// The crate's `engine` module (`OUT_DIR/ferreal_engine.rs`); `register` is the symbol of
    /// the function C++ hands the table of thunks to.
    pub fn rust(&self, register: &str) -> String {
        let mut o = String::from(
            "// Generated by ferreal-build from Unreal's reflection data (ferreal::engine!): do not edit.\n",
        );
        if self.classes.is_empty() && self.structs.is_empty() && self.enums.is_empty() {
            return o;
        }
        if !self.functions.is_empty() {
            o.push_str("\n#[doc(hidden)]\n#[repr(C)]\npub struct EngineTable {\n");
            for (i, f) in self.functions.iter().enumerate() {
                let mut args = vec!["::ferreal::ffi::RawObj".to_string()];
                args.extend(f.params.iter().map(|p| p.ty.rust_carrier_out()));
                let ret = f
                    .ret
                    .as_ref()
                    .map_or_else(|| "()".to_string(), |r| r.ty.rust_carrier_in());
                let _ = writeln!(
                    o,
                    "    /// `{}::{}`\n    pub f{i}: extern \"C\" fn({}) -> {ret},",
                    f.class,
                    f.function.cpp,
                    args.join(", ")
                );
            }
            o.push_str("}\n\n#[doc(hidden)]\npub static ENGINE: ::ferreal::ffi::Table<EngineTable> = ::ferreal::ffi::Table::new(\"the engine bindings\");\n\n");
            let _ = writeln!(
                o,
                "#[doc(hidden)]\n#[unsafe(no_mangle)]\npub extern \"C\" fn {register}(table: *const EngineTable) {{\n    ENGINE.set(table);\n}}"
            );
        }

        // Enums.
        for e in &self.enums {
            let name = enum_rust_name(self, &e.cpp);
            o.push('\n');
            let _ = writeln!(o, "/// `{}` (`{}`).", e.cpp, e.path);
            if let Some(d) = &e.doc {
                o.push_str("///\n");
                doc(&mut o, "", d);
            }
            let value_type = enum_value_type(e);
            let _ = writeln!(
                o,
                "#[doc(alias = {:?})]\n#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]\n#[repr(transparent)]\npub struct {name}(pub {});\n\nimpl {name} {{",
                e.cpp,
                value_type.rust()
            );
            let mut seen = BTreeSet::new();
            for v in &e.values {
                // -1: a value Unreal Header Tool could not evaluate; values past the type are
                // `_MAX`-style markers.
                if v.value == -1 || !value_type.holds(v.value) {
                    continue;
                }
                let c = constant_name(&v.name);
                if seen.insert(c.clone()) {
                    let _ = writeln!(o, "    pub const {c}: {name} = {name}({});", v.value);
                }
            }
            let t = value_type.rust();
            let _ = writeln!(
                o,
                "}}\n\nimpl ::ferreal::Carry for {name} {{\n    type In = {t};\n    type Out = {t};\n\n    fn from_in(c: {t}) -> Self {{\n        {name}(c)\n    }}\n\n    fn to_out(&self) -> {t} {{\n        self.0\n    }}\n}}"
            );
        }

        // Structs.
        for p in &self.structs {
            self.rust_struct(&mut o, p);
        }

        // Classes.
        for c in &self.classes {
            let m = &self.markers[&c.cpp];
            let (marker, name) = (&m.path, &m.name);
            o.push('\n');
            if m.generated {
                let _ = writeln!(o, "/// `{}` (`{}`).", c.cpp, c.path);
                if let Some(d) = &c.doc {
                    o.push_str("///\n");
                    doc(&mut o, "", d);
                }
                if self.bound_classes.contains(&c.cpp) && self.has_instance_functions(&c.cpp) {
                    let _ = writeln!(o, "///\n/// Its functions: [`{name}Api`].");
                }
                let _ = writeln!(
                    o,
                    "#[doc(alias = {:?})]\npub enum {marker} {{}}\n\nimpl ::ferreal::Class for {marker} {{\n    const CPP: &'static str = {:?};\n}}",
                    c.cpp, c.cpp
                );
            } else {
                let _ = writeln!(o, "// `{}` (`{}`): the crate's {marker}.", c.cpp, c.path);
            }
            for a in self.ancestors(&c.cpp) {
                let _ = writeln!(
                    o,
                    "impl ::ferreal::IsA<{}> for {marker} {{}}",
                    self.path(&a)
                );
            }
            if !self.bound_classes.contains(&c.cpp) {
                continue;
            }
            let instance: Vec<(usize, &Bound)> = self
                .functions
                .iter()
                .enumerate()
                .filter(|(_, f)| f.class == c.cpp && !f.function.is_static)
                .collect();
            let statics: Vec<(usize, &Bound)> = self
                .functions
                .iter()
                .enumerate()
                .filter(|(_, f)| f.class == c.cpp && f.function.is_static)
                .collect();
            if !instance.is_empty() {
                let _ = writeln!(
                    o,
                    "\n/// The functions of `{}` (and of the classes deriving from it).\npub trait {name}Api: ::ferreal::AsObject {{",
                    c.cpp
                );
                for (i, f) in &instance {
                    self.rust_function(&mut o, *i, f, true);
                }
                let _ = writeln!(
                    o,
                    "}}\n\nimpl<T: ::ferreal::AsObject + ::ferreal::IsA<{marker}>> {name}Api for T {{}}"
                );
            }
            if !statics.is_empty() {
                let _ = writeln!(
                    o,
                    "\n/// The static functions of `{}`.\nimpl {marker} {{",
                    c.cpp
                );
                for (i, f) in &statics {
                    self.rust_function(&mut o, *i, f, false);
                }
                o.push_str("}\n");
            }
        }

        if !self.skipped.is_empty() {
            let _ = writeln!(
                o,
                "\n// Left out ({}): functions whose parameters Ferreal does not pass yet.",
                self.skipped.len()
            );
            for (f, why) in &self.skipped {
                let _ = writeln!(o, "//   {f}: {why}");
            }
        }
        o
    }

    /// A struct's copy, its carriers, and its `Carry` between them.
    fn rust_struct(&self, o: &mut String, p: &StructPlan) {
        let cpp = &p.meta.cpp;
        let name = &self.struct_names[cpp];
        o.push('\n');
        let _ = writeln!(o, "/// `{cpp}` (`{}`): a copy of its fields.", p.meta.path);
        if let Some(d) = &p.meta.doc {
            o.push_str("///\n");
            doc(o, "", d);
        }
        if !p.left_out.is_empty() {
            o.push_str("///\n/// Not copied (passed to C++, they keep their defaults):\n");
            for (f, why) in &p.left_out {
                let _ = writeln!(o, "/// - `{f}`: {why}");
            }
        }
        let _ = writeln!(
            o,
            "#[doc(alias = {cpp:?})]\n#[derive(Clone, Debug, Default, PartialEq)]\npub struct {name} {{"
        );
        for f in &p.fields {
            let _ = writeln!(o, "    /// `{}`", f.cpp);
            if let Some(d) = &f.doc {
                o.push_str("    ///\n");
                doc(o, "    ", d);
            }
            let _ = writeln!(o, "    pub {}: {},", f.rust, f.ty.rust_value(self));
        }
        o.push_str("}\n");
        for (suffix, inbound) in [("In", true), ("Out", false)] {
            let _ = writeln!(
                o,
                "\n#[doc(hidden)]\n#[repr(C)]\n#[derive(Clone, Copy)]\npub struct {cpp}{suffix} {{"
            );
            for f in &p.fields {
                let t = if inbound {
                    f.ty.rust_carrier_in()
                } else {
                    f.ty.rust_carrier_out()
                };
                let _ = writeln!(o, "    pub {}: {t},", f.rust);
            }
            o.push_str("}\n");
        }
        let from: Vec<String> = p
            .fields
            .iter()
            .map(|f| {
                let v = f.ty.rust_from_carrier(self, &format!("c.{}", f.rust));
                format!("            {}: {v},", f.rust)
            })
            .collect();
        let to: Vec<String> = p
            .fields
            .iter()
            .map(|f| {
                let v = match f.ty {
                    PTy::Str | PTy::Name | PTy::Struct(_) => format!("&self.{}", f.rust),
                    _ => format!("self.{}", f.rust),
                };
                format!("            {}: {},", f.rust, f.ty.rust_to_carrier(&v))
            })
            .collect();
        let _ = writeln!(
            o,
            "\nimpl ::ferreal::Carry for {name} {{\n    type In = {cpp}In;\n    type Out = {cpp}Out;\n\n    fn from_in(c: {cpp}In) -> Self {{\n        {name} {{\n{}\n        }}\n    }}\n\n    fn to_out(&self) -> {cpp}Out {{\n        {cpp}Out {{\n{}\n        }}\n    }}\n}}",
            from.join("\n"),
            to.join("\n")
        );
    }

    /// A struct's carriers in C++, and its conversions to and from them.
    fn cpp_struct(&self, o: &mut String, p: &StructPlan) {
        let cpp = &p.meta.cpp;
        let _ = writeln!(o, "\n\t// {cpp}: the fields Rust copies.");
        for (suffix, inbound) in [("In", true), ("Out", false)] {
            let _ = writeln!(o, "\tstruct FStruct_{cpp}_{suffix}\n\t{{");
            for f in &p.fields {
                let _ = writeln!(o, "\t\t{} {};", f.ty.cpp_carrier(inbound), f.cpp);
            }
            o.push_str("\t};\n");
        }
        let _ = writeln!(
            o,
            "\n\tinline FStruct_{cpp}_In ToRust_{cpp}(const {cpp}& V)\n\t{{\n\t\tFStruct_{cpp}_In C;"
        );
        for f in &p.fields {
            let _ = writeln!(
                o,
                "\t\tC.{} = {};",
                f.cpp,
                f.ty.cpp_to_carrier(&format!("V.{}", f.cpp))
            );
        }
        o.push_str("\t\treturn C;\n\t}\n");
        let _ = writeln!(
            o,
            "\n\tinline {cpp} FromRust_{cpp}(const FStruct_{cpp}_Out& C)\n\t{{\n\t\t{cpp} V;"
        );
        for f in &p.fields {
            let e = format!("C.{}", f.cpp);
            // Fields are of their own types: an object's class, an enum, a struct deriving
            // from a math type (`FVector_NetQuantize`).
            let value = match &f.ty {
                PTy::Obj(c) | PTy::Weak(c) => {
                    let path = self
                        .classes
                        .iter()
                        .find(|m| &m.cpp == c)
                        .map_or("", |m| m.path.as_str());
                    format!("Ferreal::FromObjOf<{c}>({e}, TEXT(\"{path}\"))")
                }
                t if matches!(t, PTy::Enum { .. }) || t.is_math() => format!(
                    "static_cast<decltype(V.{})>({})",
                    f.cpp,
                    f.ty.cpp_from_carrier(&e)
                ),
                t => t.cpp_from_carrier(&e),
            };
            let _ = writeln!(o, "\t\tV.{} = {value};", f.cpp);
        }
        o.push_str("\t\treturn V;\n\t}\n");
    }

    /// The headers of the structs the bindings pass, and of the classes their fields point
    /// to (converting a field casts to its class, which a struct's header may only declare).
    pub fn struct_headers(&self) -> impl Iterator<Item = &str> {
        let classes = self.structs.iter().flat_map(|p| {
            p.fields.iter().filter_map(|f| match &f.ty {
                PTy::Obj(c) | PTy::Weak(c) => self
                    .classes
                    .iter()
                    .find(|m| &m.cpp == c)
                    .and_then(|m| m.header.as_deref()),
                _ => None,
            })
        });
        self.structs
            .iter()
            .filter_map(|p| p.meta.header.as_deref())
            .chain(classes)
    }

    fn has_instance_functions(&self, cpp: &str) -> bool {
        self.functions
            .iter()
            .any(|f| f.class == cpp && !f.function.is_static)
    }

    /// The classes `cpp` is (itself, then its ancestors) that have markers.
    fn ancestors(&self, cpp: &str) -> Vec<String> {
        let by_cpp: HashMap<&str, &ClassMeta> =
            self.classes.iter().map(|c| (c.cpp.as_str(), c)).collect();
        let mut out = Vec::new();
        let mut at = Some(cpp.to_string());
        while let Some(c) = at {
            let Some(m) = by_cpp.get(c.as_str()) else {
                break;
            };
            at = m.parent.clone();
            out.push(c);
        }
        out
    }

    fn rust_function(&self, o: &mut String, index: usize, f: &Bound, instance: bool) {
        let fm = &f.function;
        o.push('\n');
        let _ = writeln!(o, "    /// `{}::{}`", f.class, fm.cpp);
        if let Some(d) = &fm.doc {
            o.push_str("    ///\n");
            doc(o, "    ", d);
        }
        if fm.editor_only {
            o.push_str("    ///\n    /// Editor only: in packaged games it does nothing (and logs an error).\n");
        }
        if let Some(why) = &fm.deprecated {
            let _ = writeln!(o, "    #[deprecated(note = {:?})]", why);
        }
        // Found by its C++ (and Blueprint) name too.
        let _ = writeln!(o, "    #[doc(alias = {:?})]", fm.cpp);
        let mut generics = Vec::new();
        let mut params = Vec::new();
        if instance {
            params.push("&self".to_string());
        }
        for (k, p) in f.params.iter().enumerate() {
            let generic = format!("P{k}");
            if let PTy::Obj(c) = &p.ty {
                generics.push(format!("{generic}: ::ferreal::IsA<{}>", self.path(c)));
            }
            params.push(format!("{}: {}", p.rust, p.ty.rust_param(self, &generic)));
        }
        let ret = f
            .ret
            .as_ref()
            .map(|r| format!(" -> {}", r.ty.rust_value(self)))
            .unwrap_or_default();
        let generics = if generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", generics.join(", "))
        };
        let vis = if instance { "" } else { "pub " };
        let _ = writeln!(
            o,
            "    {vis}fn {}{generics}({}){ret} {{",
            f.rust,
            params.join(", ")
        );
        let mut args = vec![if instance {
            "::ferreal::AsObject::raw_object(self)".to_string()
        } else {
            "::ferreal::ffi::RawObj::NULL".to_string()
        }];
        args.extend(f.params.iter().map(|p| p.ty.rust_to_carrier(&p.rust)));
        let call = format!("(ENGINE.get().f{index})({})", args.join(", "));
        match &f.ret {
            None => {
                let _ = writeln!(o, "        {call}");
            }
            Some(r) => {
                let _ = writeln!(
                    o,
                    "        let r = {call};\n        {}",
                    r.ty.rust_from_carrier(self, "r")
                );
            }
        }
        o.push_str("    }\n");
    }

    /// The C++ the module's `.cpp` needs: declarations (in `Ferreal::<M>::Detail`), the
    /// thunks and the table (in the same namespace).
    pub fn cpp_table_type(&self) -> String {
        let mut o = String::from("\n\tstruct FEngineTable\n\t{\n");
        for (i, f) in self.functions.iter().enumerate() {
            let mut args = vec!["void* Object".to_string()];
            args.extend(
                f.params
                    .iter()
                    .map(|p| format!("{} In{}", p.ty.cpp_carrier(false), p.cpp)),
            );
            let ret = f
                .ret
                .as_ref()
                .map_or_else(|| "void".to_string(), |r| r.ty.cpp_carrier(true));
            let _ = writeln!(o, "\t\t{ret} (*F{i})({});", args.join(", "));
        }
        o.push_str("\t};\n");
        o
    }

    pub fn cpp_thunks(&self) -> String {
        let mut o = String::new();
        for (i, f) in self.functions.iter().enumerate() {
            let fm = &f.function;
            let class_path = &self
                .classes
                .iter()
                .find(|c| c.cpp == f.class)
                .expect("a bound class")
                .path;
            let _ = writeln!(
                o,
                "\n\t// {}::{}\n\tstruct FEngineParms{i}\n\t{{",
                f.class, fm.cpp
            );
            for p in f.params.iter().chain(&f.ret) {
                let _ = writeln!(o, "\t\t{} {};", p.ty.cpp_member(), p.cpp);
            }
            o.push_str("\t};\n");
            let mut args = vec!["void* Object".to_string()];
            args.extend(
                f.params
                    .iter()
                    .map(|p| format!("{} In{}", p.ty.cpp_carrier(false), p.cpp)),
            );
            let ret = f
                .ret
                .as_ref()
                .map_or_else(|| "void".to_string(), |r| r.ty.cpp_carrier(true));
            let layout: Vec<String> = f
                .params
                .iter()
                .chain(&f.ret)
                .map(|p| {
                    format!(
                        "Ferreal::FEngineParam(Parms, Parms.{0}, TEXT(\"{0}\"), TEXT(\"{1}\"), {2})",
                        p.cpp,
                        p.property,
                        p.strukt
                            .as_ref()
                            .map_or_else(|| "nullptr".to_string(), |s| format!("TEXT(\"{s}\")"))
                    )
                })
                .collect();
            let fail = if f.ret.is_some() {
                "return {};"
            } else {
                "return;"
            };
            let _ = writeln!(
                o,
                "\n\tstatic {ret} Engine{i}({})\n\t{{\n\t\tstatic Ferreal::FEngineFunction Function(TEXT(\"{class_path}\"), TEXT(\"{}\"), {});\n\t\tFEngineParms{i} Parms{{}};\n\t\tif (!Function.Ready([&] {{ return Ferreal::FEngineLayout{{static_cast<int32>(sizeof(Parms)), {{{}}}}}; }}))\n\t\t{{\n\t\t\t{fail}\n\t\t}}",
                args.join(", "),
                fm.name,
                if fm.is_static { "true" } else { "false" },
                layout.join(", ")
            );
            for p in &f.params {
                let _ = writeln!(
                    o,
                    "\t\tParms.{} = {};",
                    p.cpp,
                    p.ty.cpp_from_carrier(&format!("In{}", p.cpp))
                );
            }
            let _ = writeln!(
                o,
                "\t\tFunction.Call(static_cast<UObject*>(Object), &Parms);"
            );
            if let Some(r) = &f.ret {
                let _ = writeln!(
                    o,
                    "\t\treturn {};",
                    r.ty.cpp_to_carrier(&format!("Parms.{}", r.cpp))
                );
            }
            o.push_str("\t}\n");
        }
        let entries: Vec<String> = (0..self.functions.len())
            .map(|i| format!("&Engine{i}"))
            .collect();
        let _ = writeln!(
            o,
            "\n\tstatic const FEngineTable EngineTable = {{{}}};",
            entries.join(", ")
        );
        o
    }

    /// The lock file's contents.
    pub fn lock_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.lock).expect("serializable");
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(snake("K2_SetActorLocation"), "k2_set_actor_location");
        assert_eq!(snake("GetHUD"), "get_hud");
        assert_eq!(snake("SetUIScale"), "set_ui_scale");
        assert_eq!(snake("SetActorScale3D"), "set_actor_scale3d");
        assert_eq!(snake("Get2DPosition"), "get2d_position");
        assert_eq!(snake("Concat_StrStr"), "concat_str_str");
        assert_eq!(param_name("bNewHidden", true), "new_hidden");
        assert_eq!(param_name("bNewHidden", false), "b_new_hidden");
        assert_eq!(param_name("Type", false), "r#type");
        assert_eq!(param_name("Self", false), "self_");
        assert_eq!(constant_name("NoCollision"), "NO_COLLISION");
    }

    fn ty(kind: &str, property: &str) -> TypeMeta {
        TypeMeta {
            kind: kind.into(),
            property: property.into(),
            ..TypeMeta::default()
        }
    }

    fn param(name: &str, t: TypeMeta) -> ParamMeta {
        ParamMeta {
            name: name.into(),
            ty: t,
            ..ParamMeta::default()
        }
    }

    fn sample() -> Metadata {
        let object = ClassMeta {
            cpp: "UObject".into(),
            path: "/Script/CoreUObject.Object".into(),
            ..ClassMeta::default()
        };
        let mut owner = ty("object", "ObjectProperty");
        owner.class = Some("AActor".into());
        let mut location = ty("struct", "StructProperty");
        location.strukt = Some("FVector".into());
        let mut ret_owner = param("ReturnValue", owner.clone());
        ret_owner.ret = true;
        let mut hit = param("SweepHitResult", ty("struct", "StructProperty"));
        hit.out = true;
        let actor = ClassMeta {
            cpp: "AActor".into(),
            path: "/Script/Engine.Actor".into(),
            parent: Some("UObject".into()),
            functions: vec![
                FunctionMeta {
                    cpp: "SetActorHiddenInGame".into(),
                    name: "SetActorHiddenInGame".into(),
                    params: vec![param("bNewHidden", ty("bool", "BoolProperty"))],
                    ..FunctionMeta::default()
                },
                FunctionMeta {
                    cpp: "GetOwner".into(),
                    name: "GetOwner".into(),
                    params: vec![ret_owner],
                    ..FunctionMeta::default()
                },
                FunctionMeta {
                    cpp: "K2_SetActorLocation".into(),
                    name: "K2_SetActorLocation".into(),
                    params: vec![param("NewLocation", location), hit],
                    ..FunctionMeta::default()
                },
            ],
            ..ClassMeta::default()
        };
        let spinner = ClassMeta {
            cpp: "ASpinner".into(),
            path: "/Script/Hello.Spinner".into(),
            parent: Some("AActor".into()),
            functions: vec![FunctionMeta {
                cpp: "Spin".into(),
                name: "Spin".into(),
                ..FunctionMeta::default()
            }],
            ..ClassMeta::default()
        };
        Metadata {
            format: FORMAT.into(),
            version: FORMAT_VERSION,
            classes: vec![object, actor, spinner],
            structs: vec![],
            enums: vec![],
        }
    }

    #[test]
    fn resolves_and_generates() {
        let req = [Request {
            cpp: "AActor".into(),
            at: "src/lib.rs:3".into(),
        }];
        let handles = [Handle {
            rust_path: "crate::Spinner".into(),
            cpp: "ASpinner".into(),
            own: true,
        }];
        let b = resolve(&Source::Exported(sample()), &req, &handles, &[]).unwrap();
        assert_eq!(b.marker("AActor"), Some("Actor"));
        assert_eq!(b.functions.len(), 2);
        assert_eq!(b.skipped.len(), 1);
        let rust = b.rust("ferreal_Hello_engine_register");
        assert!(rust.contains("pub trait ActorApi: ::ferreal::AsObject"));
        assert!(rust.contains("fn set_actor_hidden_in_game(&self, new_hidden: bool)"));
        assert!(
            rust.contains("fn get_owner(&self) -> ::core::option::Option<::ferreal::Obj<Actor>>")
        );
        assert!(rust.contains("impl ::ferreal::IsA<Object> for Actor {}"));
        assert!(rust.contains("impl ::ferreal::IsA<Actor> for crate::Spinner {}"));
        assert!(!rust.contains("SpinnerApi"));
        assert!(rust.contains("AActor::K2_SetActorLocation: its output parameter SweepHitResult"));
        let cpp = b.cpp_thunks();
        assert!(cpp.contains("bool bNewHidden;"));
        assert!(cpp.contains("UObject* ReturnValue;"));
        assert!(cpp.contains("TEXT(\"/Script/Engine.Actor\"), TEXT(\"GetOwner\"), false"));
        assert!(b.lock_json().contains("\"SetActorHiddenInGame\""));
    }

    #[test]
    fn copies_structs() {
        let mut m = sample();
        let field = |name: &str, t: TypeMeta| FieldMeta {
            name: name.into(),
            ty: t,
            is_public: true,
            ..FieldMeta::default()
        };
        let mut location = ty("struct", "StructProperty");
        location.strukt = Some("FVector_NetQuantize".into());
        let mut component = ty("weak", "WeakObjectProperty");
        component.class = Some("AActor".into());
        let mut private = field("FaceIndex", ty("int32", "IntProperty"));
        private.is_public = false;
        m.structs = vec![
            StructMeta {
                cpp: "FVector".into(),
                noexport: true,
                ..StructMeta::default()
            },
            StructMeta {
                cpp: "FVector_NetQuantize".into(),
                parent: Some("FVector".into()),
                ..StructMeta::default()
            },
            StructMeta {
                cpp: "FHitResult".into(),
                path: "/Script/Engine.HitResult".into(),
                header: Some("Engine/HitResult.h".into()),
                fields: vec![
                    field("bBlockingHit", ty("bool", "BoolProperty")),
                    field("Location", location),
                    field("Actor", component),
                    field("BoneName", ty("name", "NameProperty")),
                    private,
                ],
                ..StructMeta::default()
            },
        ];
        let mut hit = ty("struct", "StructProperty");
        hit.strukt = Some("FHitResult".into());
        let mut ret = param("ReturnValue", hit.clone());
        ret.ret = true;
        m.classes[1].functions.push(FunctionMeta {
            cpp: "LastHit".into(),
            name: "LastHit".into(),
            params: vec![param("Previous", hit), ret],
            ..FunctionMeta::default()
        });
        let req = [Request {
            cpp: "AActor".into(),
            at: "src/lib.rs:3".into(),
        }];
        let b = resolve(&Source::Exported(m), &req, &[], &[]).unwrap();
        let rust = b.rust("ferreal_Hello_engine_register");
        assert!(rust.contains("pub struct HitResult {"), "{rust}");
        assert!(rust.contains("pub blocking_hit: bool,"));
        assert!(rust.contains("pub location: ::ferreal::Vector3,"));
        assert!(rust.contains("pub actor: ::core::option::Option<::ferreal::Obj<Actor>>,"));
        assert!(rust.contains("/// - `FaceIndex`: not public"));
        assert!(rust.contains("fn last_hit(&self, previous: &HitResult) -> HitResult"));
        let cpp = b.cpp_structs() + &b.cpp_table_type() + &b.cpp_thunks();
        assert!(
            cpp.contains("C.Actor = Ferreal::ToObj(V.Actor.Get());"),
            "{cpp}"
        );
        assert!(cpp.contains(
            "V.Location = static_cast<decltype(V.Location)>(Ferreal::FromVec3(C.Location));"
        ));
        assert!(cpp.contains("C.BoneName = Ferreal::KeepString(V.BoneName.ToString());"));
        assert!(cpp.contains("Parms.Previous = FromRust_FHitResult(InPrevious);"));
        assert!(cpp.contains("return ToRust_FHitResult(Parms.ReturnValue);"));
        assert_eq!(
            b.struct_headers().collect::<Vec<_>>(),
            ["Engine/HitResult.h"]
        );
        // The lock keeps what resolving needed.
        let lock = b.lock_json();
        assert!(lock.contains("\"FVector_NetQuantize\"") && lock.contains("\"FHitResult\""));
    }

    #[test]
    fn errors() {
        let req = [Request {
            cpp: "AActr".into(),
            at: "src/lib.rs:3".into(),
        }];
        let e = resolve(&Source::Exported(sample()), &req, &[], &[]).unwrap_err();
        assert!(e[0].contains("AActr is not a reflected class"));
        let e = resolve(&Source::None, &req, &[], &[]).unwrap_err();
        assert!(e[0].contains(NEEDS_METADATA));
        let e = resolve(&Source::Lock(sample()), &req, &[], &[]).unwrap_err();
        assert!(e[0].contains(NEEDS_METADATA));
        assert!(resolve(&Source::None, &[], &[], &[]).unwrap().is_empty());
    }
}
