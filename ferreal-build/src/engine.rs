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
    #[serde(default)]
    pub enums: Vec<EnumMeta>,
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
    /// An object of this C++ class.
    Obj(String),
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
    /// Functions left out, with why (shown in the generated file).
    skipped: Vec<(String, String)>,
    /// What the lock file keeps.
    pub lock: Metadata,
}

impl Bindings {
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
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
pub fn resolve(
    source: &Source,
    requests: &[Request],
    handles: &[Handle],
) -> Result<Bindings, Vec<String>> {
    let (meta, exported) = match source {
        Source::Exported(m) => (m, true),
        Source::Lock(m) => (m, false),
        Source::None => {
            if requests.is_empty() {
                return Ok(Bindings::default());
            }
            return Err(vec![format!(
                "{}: ferreal::engine! names engine classes, but there is no reflection data yet. Enable the Ferreal plugin and build the Unreal project: Unreal Header Tool exports the data during that build, and the next build generates the bindings ({NEEDS_METADATA}). Outside Unreal builds, commit the {} that build writes, or set {METADATA_ENV} to the exporter's FerrealEngine.json.",
                requests[0].at, LOCK_FILE
            )]);
        }
    };
    let classes: HashMap<&str, &ClassMeta> =
        meta.classes.iter().map(|c| (c.cpp.as_str(), c)).collect();
    let enums: HashMap<&str, &EnumMeta> = meta.enums.iter().map(|e| (e.cpp.as_str(), e)).collect();
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
            match bind(class, f, &classes, &enums) {
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
        bound_classes: bound
            .into_iter()
            .filter(|c| !own.contains(c.as_str()))
            .collect(),
        skipped,
        lock,
    })
}

/// How a function binds, or why it does not.
fn bind(
    class: &ClassMeta,
    f: &FunctionMeta,
    classes: &HashMap<&str, &ClassMeta>,
    enums: &HashMap<&str, &EnumMeta>,
) -> Result<Bound, String> {
    let mut params = Vec::new();
    let mut ret = None;
    let mut used = BTreeSet::new();
    for p in &f.params {
        if p.out && !p.is_const {
            return Err(format!("its output parameter {}", p.name));
        }
        let ty = param_type(&p.ty, classes, enums).map_err(|t| format!("{} is {t}", p.name))?;
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

fn param_type(
    t: &TypeMeta,
    classes: &HashMap<&str, &ClassMeta>,
    enums: &HashMap<&str, &EnumMeta>,
) -> Result<PTy, String> {
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
            let Some(e) = enums.get(name.as_str()) else {
                return Err(format!("enum {name}, which was not exported"));
            };
            let value = enum_value_type(e);
            // A TEnumAsByte (a ByteProperty) is a byte whatever the enum's type; an
            // EnumProperty holds the enum's own type, which an old namespaced enum leaves to
            // the compiler.
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
        "struct" => match t.strukt.as_deref() {
            Some("FVector") => PTy::Vector,
            Some("FVector2D") => PTy::Vector2D,
            Some("FLinearColor") => PTy::LinearColor,
            Some("FRotator") => PTy::Rotator,
            s => return Err(format!("struct {}", s.unwrap_or("?"))),
        },
        "object" => {
            let c = t.class.clone().unwrap_or_else(|| "UObject".into());
            if !classes.contains_key(c.as_str()) {
                return Err(format!("an object of {c}, which was not exported"));
            }
            PTy::Obj(c)
        }
        k => return Err(format!("a {k} ({})", t.property)),
    })
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
    /// The Rust type of a parameter (`generic`: the type parameter of an object's class).
    fn rust_param(&self, b: &Bindings, generic: &str) -> String {
        match self {
            PTy::Str | PTy::Name => "&str".into(),
            PTy::Obj(_) => format!("::core::option::Option<::ferreal::Obj<{generic}>>"),
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
            PTy::Obj(c) => format!("::core::option::Option<::ferreal::Obj<{}>>", b.path(c)),
        }
    }

    /// The carrier Rust passes to C++.
    fn rust_carrier_out(&self) -> String {
        match self {
            PTy::Str | PTy::Name => "::ferreal::ffi::Str8".into(),
            PTy::Enum { value, .. } => value.rust().into(),
            PTy::Obj(_) => "::ferreal::ffi::RawObj".into(),
            _ => self.rust_value_plain(),
        }
    }

    /// The carrier C++ returns to Rust.
    fn rust_carrier_in(&self) -> String {
        match self {
            PTy::Str | PTy::Name => "::ferreal::ffi::Str16".into(),
            PTy::Enum { value, .. } => value.rust().into(),
            PTy::Obj(_) => "::ferreal::ffi::RawObj".into(),
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
            _ => unreachable!("not a plain carrier"),
        }
        .into()
    }

    /// Rust argument `v` as its carrier.
    fn rust_to_carrier(&self, v: &str) -> String {
        match self {
            PTy::Str | PTy::Name => format!("::ferreal::ffi::Str8::new({v})"),
            PTy::Enum { .. } => format!("{v}.0"),
            PTy::Obj(_) => format!("{v}.map_or(::ferreal::ffi::RawObj::NULL, |o| o.raw())"),
            _ => v.into(),
        }
    }

    /// Carrier `v` returned by C++ as the Rust value.
    fn rust_from_carrier(&self, b: &Bindings, v: &str) -> String {
        match self {
            PTy::Str | PTy::Name => format!("{v}.to_string_lossy()"),
            PTy::Enum { name, .. } => format!("{}({v})", enum_rust_name(b, name)),
            PTy::Obj(_) => format!("::ferreal::Obj::from_raw({v})"),
            _ => v.into(),
        }
    }

    /// The type of the parameter's member in the C++ struct of parameters.
    fn cpp_member(&self) -> &'static str {
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
            PTy::Obj(_) => "UObject*",
        }
    }

    /// The C++ carrier from Rust (`inbound` false) or to Rust (`inbound` true).
    fn cpp_carrier(&self, inbound: bool) -> &'static str {
        match self {
            PTy::Str | PTy::Name if inbound => "Ferreal::FStr16",
            PTy::Str | PTy::Name => "Ferreal::FStr8",
            PTy::Vector => "Ferreal::FVec3",
            PTy::Vector2D => "Ferreal::FVec2",
            PTy::LinearColor => "Ferreal::FColor4",
            PTy::Rotator => "Ferreal::FRot",
            PTy::Obj(_) => "void*",
            PTy::Enum { value, .. } => value.cpp(),
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
            PTy::Obj(_) => format!("static_cast<UObject*>({e})"),
            PTy::Enum { member, .. } => format!("static_cast<{}>({e})", member.cpp()),
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
            PTy::Obj(_) => format!("Ferreal::ToObj({e})"),
            PTy::Enum { value, .. } => format!("static_cast<{}>({e})", value.cpp()),
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
        if self.classes.is_empty() {
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
            o.push_str("}\n");
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
            let ret = f.ret.as_ref().map_or("void", |r| r.ty.cpp_carrier(true));
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
            let ret = f.ret.as_ref().map_or("void", |r| r.ty.cpp_carrier(true));
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
        let b = resolve(&Source::Exported(sample()), &req, &handles).unwrap();
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
    fn errors() {
        let req = [Request {
            cpp: "AActr".into(),
            at: "src/lib.rs:3".into(),
        }];
        let e = resolve(&Source::Exported(sample()), &req, &[]).unwrap_err();
        assert!(e[0].contains("AActr is not a reflected class"));
        let e = resolve(&Source::None, &req, &[]).unwrap_err();
        assert!(e[0].contains(NEEDS_METADATA));
        let e = resolve(&Source::Lock(sample()), &req, &[]).unwrap_err();
        assert!(e[0].contains(NEEDS_METADATA));
        assert!(resolve(&Source::None, &[], &[]).unwrap().is_empty());
    }
}
