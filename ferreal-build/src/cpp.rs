//! Writes the C++ half of the glue: the `UCLASS` headers, the `UENUM`s, the module header of
//! exported functions, and the implementation that calls into Rust.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use ferreal_model::{
    Base, CallSuper, Class, DefaultValue, Enum, Export, Method, MethodKind, Param, Property,
    Receiver, Symbols, Ty,
};

use crate::Collected;
use crate::engine::Bindings;

/// A class Rust refers to (`Obj<C>`): its C++ name and where it is declared.
#[derive(Clone, Debug)]
struct ClassRef {
    cpp: String,
    /// The header to include: an engine class' own, `Ferreal/<Name>.h` for a generated one.
    include: Option<String>,
    generated: bool,
}

/// A generated file: its path under the module directory and contents.
pub struct File {
    pub path: String,
    pub contents: String,
}

pub struct Gen<'a> {
    module: &'a str,
    api: &'a str,
    source: &'a str,
    symbols: Symbols,
    model: &'a Collected,
    enums: HashMap<&'a str, &'a Enum>,
    classes: HashMap<String, ClassRef>,
    engine: &'a Bindings,
}

/// How a parameter is declared in C++.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    /// A `UFUNCTION`, an override: Unreal's reflection rules (`TArray` for arrays).
    Reflected,
    /// A plain C++ function (exports, C++ base methods): views for arrays.
    Plain,
}

fn header_name(cpp: &str) -> String {
    let bare = cpp.get(1..).unwrap_or(cpp);
    format!("{bare}.h")
}

fn doc_comment(out: &mut String, indent: &str, doc: &[String]) {
    if doc.is_empty() {
        return;
    }
    let text = doc.join("\n").replace("*/", "* /");
    if doc.len() == 1 {
        let _ = writeln!(out, "{indent}/** {text} */");
    } else {
        let _ = writeln!(out, "{indent}/**");
        for line in text.lines() {
            let _ = writeln!(out, "{indent} * {line}");
        }
        let _ = writeln!(out, "{indent} */");
    }
}

fn cpp_string(s: &str) -> String {
    let mut out = String::from("TEXT(\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push_str("\")");
    out
}

/// A `cpp!` block's text, its lines indented with `inner` (their common indentation
/// removed) and its closing line with `close`. A single line is kept as written.
fn reindent(text: &str, inner: &str, close: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return text.to_string();
    }
    let common = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = lines[0].trim_end().to_string();
    for l in &lines[1..] {
        out.push('\n');
        if !l.trim().is_empty() {
            out.push_str(inner);
            out.push_str(l.get(common..).unwrap_or(l.trim_start()).trim_end());
        }
    }
    // The last line holds the indentation before the closing brace.
    if lines.last().is_some_and(|l| l.trim().is_empty()) {
        out.push_str(close);
    }
    out
}

/// A float literal: `24` → `24.0f`.
fn float(n: &str, f32_suffix: bool) -> String {
    let mut s = n.to_string();
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    if f32_suffix {
        s.push('f');
    }
    s
}

impl<'a> Gen<'a> {
    pub fn new(
        module: &'a str,
        api: &'a str,
        source: &'a str,
        model: &'a Collected,
        engine: &'a Bindings,
    ) -> Self {
        let mut classes = HashMap::new();
        // The engine bindings' markers (`Obj<engine::Actor>`), unless the crate declares a
        // class of the same name.
        for (name, cpp, header) in engine.marker_classes() {
            classes.insert(
                name.to_string(),
                ClassRef {
                    cpp: cpp.to_string(),
                    include: header.map(str::to_string),
                    generated: false,
                },
            );
        }
        for e in &model.engine {
            classes.insert(
                e.decl.name.to_string(),
                ClassRef {
                    cpp: e.decl.cpp_class.clone(),
                    include: e.decl.include.clone(),
                    generated: false,
                },
            );
        }
        for c in &model.classes {
            classes.insert(
                c.class.name.clone(),
                ClassRef {
                    cpp: c.class.cpp_name.clone(),
                    include: Some(format!("Ferreal/{}", header_name(&c.class.cpp_name))),
                    generated: true,
                },
            );
        }
        let enums = model
            .enums
            .iter()
            .map(|e| (e.item.name.as_str(), &e.item))
            .collect();
        Gen {
            module,
            api,
            source,
            symbols: Symbols::new(module),
            model,
            enums,
            classes,
            engine,
        }
    }

    fn class(&self, name: &str) -> &ClassRef {
        &self.classes[name]
    }

    fn enum_cpp(&self, name: &str) -> &str {
        &self.enums[name].cpp_name
    }

    /// The C++ type of a property or return value.
    fn value_type(&self, ty: &Ty) -> String {
        match ty {
            Ty::Unit => "void".into(),
            Ty::Bool => "bool".into(),
            Ty::I32 => "int32".into(),
            Ty::I64 => "int64".into(),
            Ty::U8 => "uint8".into(),
            Ty::U32 => "uint32".into(),
            Ty::F32 => "float".into(),
            Ty::F64 => "double".into(),
            Ty::String { .. } => "FString".into(),
            Ty::LinearColor => "FLinearColor".into(),
            Ty::Vector2 => "FVector2D".into(),
            Ty::Vector3 => "FVector".into(),
            Ty::Enum(e) => self.enum_cpp(e).into(),
            Ty::Obj { class, .. } => format!("{}*", self.class(class).cpp),
            Ty::ObjArray { class, .. } => format!("TArray<{}*>", self.class(class).cpp),
            Ty::Slice { prim, .. } => format!("TArray<{}>", prim.cpp()),
        }
    }

    fn property_type(&self, ty: &Ty) -> String {
        match ty {
            Ty::Obj { class, .. } => format!("TObjectPtr<{}>", self.class(class).cpp),
            Ty::ObjArray { class, .. } => format!("TArray<TObjectPtr<{}>>", self.class(class).cpp),
            _ => self.value_type(ty),
        }
    }

    fn param_type(&self, ty: &Ty, site: Site) -> String {
        match ty {
            Ty::String { .. } => "const FString&".into(),
            Ty::LinearColor => "const FLinearColor&".into(),
            Ty::Vector2 => "const FVector2D&".into(),
            Ty::Vector3 => "const FVector&".into(),
            Ty::ObjArray { class, .. } => format!("const TArray<{}*>&", self.class(class).cpp),
            Ty::Slice { prim, .. } if site == Site::Reflected => {
                format!("const TArray<{}>&", prim.cpp())
            }
            Ty::Slice { prim, .. } => format!("TConstArrayView<{}>", prim.cpp()),
            _ => self.value_type(ty),
        }
    }

    /// C++ value `e` as the carrier Rust receives.
    fn to_rust(&self, ty: &Ty, e: &str) -> String {
        match ty {
            Ty::String { .. } => format!("Ferreal::ToStr16({e})"),
            Ty::LinearColor => format!("Ferreal::ToColor4({e})"),
            Ty::Vector2 => format!("Ferreal::ToVec2({e})"),
            Ty::Vector3 => format!("Ferreal::ToVec3({e})"),
            Ty::Enum(_) => format!("static_cast<uint8>({e})"),
            Ty::Obj { .. } => format!("Ferreal::ToObj({e})"),
            Ty::ObjArray { .. } => {
                format!("static_cast<Ferreal::TSlice<void*>>(Ferreal::FObjArgs({e}))")
            }
            Ty::Slice { .. } => format!("Ferreal::ToSlice({e})"),
            _ => e.into(),
        }
    }

    /// C++ value `e` returned to Rust, kept past the end of the C++ function.
    fn keep_for_rust(&self, ty: &Ty, e: &str) -> String {
        match ty {
            Ty::String { .. } => format!("Ferreal::KeepString({e})"),
            Ty::ObjArray { .. } => format!("Ferreal::KeepObjs({e})"),
            Ty::Slice { .. } => format!("Ferreal::KeepArray({e})"),
            _ => self.to_rust(ty, e),
        }
    }

    /// Rust's carrier `e` as a C++ value; `property` for assigning a `UPROPERTY`.
    fn rust_to_cpp(&self, ty: &Ty, e: &str, property: bool) -> String {
        match ty {
            Ty::String { .. } => format!("Ferreal::ToFString({e})"),
            Ty::LinearColor => format!("Ferreal::FromColor4({e})"),
            Ty::Vector2 => format!("Ferreal::FromVec2({e})"),
            Ty::Vector3 => format!("Ferreal::FromVec3({e})"),
            Ty::Enum(n) => format!("static_cast<{}>({e})", self.enum_cpp(n)),
            Ty::Obj { class, .. } => format!("Ferreal::FromObj<{}>({e})", self.class(class).cpp),
            Ty::ObjArray { class, .. } => {
                let c = &self.class(class).cpp;
                let elem = if property {
                    format!("TObjectPtr<{c}>")
                } else {
                    format!("{c}*")
                };
                format!("Ferreal::FromObjs<{elem}, {c}>({e})")
            }
            Ty::Slice { .. } => format!("Ferreal::ToView({e})"),
            _ => e.into(),
        }
    }

    /// Rust's carrier `e`, returned by a Rust function, as the C++ value returned.
    fn returned_from_rust(&self, ty: &Ty, e: &str) -> String {
        match ty {
            Ty::Slice { .. } => format!("Ferreal::ToArray({e})"),
            _ => self.rust_to_cpp(ty, e, false),
        }
    }

    /// The C++ value of `ty` when Rust could not be called (its carrier's zero).
    fn fallback(&self, ty: &Ty) -> String {
        match ty {
            Ty::Unit => String::new(),
            Ty::Bool => "false".into(),
            Ty::Obj { .. } => "nullptr".into(),
            Ty::Enum(n) => format!("static_cast<{}>(0)", self.enum_cpp(n)),
            t if t.is_plain() => format!("static_cast<{}>(0)", self.value_type(t)),
            t => self.returned_from_rust(t, &format!("{}{{}}", t.cpp_ffi(false))),
        }
    }

    fn default_value(&self, p: &Property, d: &DefaultValue) -> Result<String, String> {
        let bad = || format!("`{}`: this default does not fit a {:?}", p.name, p.ty);
        Ok(match (&p.ty, d) {
            (Ty::Bool, DefaultValue::Bool(b)) => b.to_string(),
            (Ty::F32, DefaultValue::Number(n)) => float(n, true),
            (Ty::F64, DefaultValue::Number(n)) => float(n, false),
            (Ty::I32 | Ty::I64 | Ty::U8 | Ty::U32, DefaultValue::Number(n))
                if n.parse::<i128>().is_ok() =>
            {
                n.clone()
            }
            (Ty::String { .. }, DefaultValue::Str(s)) => cpp_string(s),
            (Ty::LinearColor, DefaultValue::Numbers(v)) if v.len() == 3 || v.len() == 4 => {
                let mut c: Vec<String> = v.iter().map(|n| float(n, true)).collect();
                if c.len() == 3 {
                    c.push("1.0f".into());
                }
                format!("FLinearColor({})", c.join(", "))
            }
            (Ty::Vector2, DefaultValue::Numbers(v)) if v.len() == 2 => {
                format!(
                    "FVector2D({}, {})",
                    float(&v[0], false),
                    float(&v[1], false)
                )
            }
            (Ty::Vector3, DefaultValue::Numbers(v)) if v.len() == 3 => {
                format!(
                    "FVector({}, {}, {})",
                    float(&v[0], false),
                    float(&v[1], false),
                    float(&v[2], false)
                )
            }
            (Ty::Enum(e), DefaultValue::Variant(v)) => {
                let en = self.enums[e.as_str()];
                if !en.variants.iter().any(|x| x.name == *v) {
                    return Err(format!("`{}`: {e} has no variant {v}", p.name));
                }
                format!("{}::{v}", en.cpp_name)
            }
            _ => return Err(bad()),
        })
    }

    fn banner(&self) -> String {
        format!(
            "// Generated by ferreal-build from the Rust crate at {}: do not edit.\n// Change the Rust and build again.\n",
            self.source
        )
    }

    /// Every file of the module.
    pub fn files(&self) -> Result<Vec<File>, String> {
        let mut files = vec![File {
            path: "Public/Ferreal/FerrealRuntime.h".into(),
            contents: include_str!("FerrealRuntime.h").into(),
        }];
        if !self.model.enums.is_empty() {
            files.push(File {
                path: format!("Public/Ferreal/{}.h", self.enums_header()),
                contents: self.enums_file(),
            });
        }
        files.push(File {
            path: format!("Public/Ferreal/Ferreal{}.h", self.module),
            contents: self.module_header(),
        });
        for c in &self.model.classes {
            files.push(File {
                path: format!("Public/Ferreal/{}", header_name(&c.class.cpp_name)),
                contents: self.class_header(&c.class)?,
            });
        }
        if !self.engine.is_empty() {
            files.push(File {
                path: "Private/Ferreal/FerrealEngine.h".into(),
                contents: include_str!("FerrealEngine.h").into(),
            });
        }
        files.push(File {
            path: format!("Private/Ferreal/Ferreal{}.cpp", self.module),
            contents: self.implementation()?,
        });
        Ok(files)
    }

    fn enums_header(&self) -> String {
        format!("Ferreal{}Enums", self.module)
    }

    fn enums_file(&self) -> String {
        let mut o = self.banner();
        let _ = writeln!(
            o,
            "\n#pragma once\n\n#include \"CoreMinimal.h\"\n#include \"{}.generated.h\"",
            self.enums_header()
        );
        for e in &self.model.enums {
            let e = &e.item;
            o.push('\n');
            doc_comment(&mut o, "", &e.doc);
            let _ = writeln!(
                o,
                "UENUM({})\nenum class {} : uint8\n{{",
                e.specifiers.join(", "),
                e.cpp_name
            );
            for v in &e.variants {
                doc_comment(&mut o, "\t", &v.doc);
                let meta = v
                    .meta
                    .as_ref()
                    .map(|m| format!(" UMETA({m})"))
                    .unwrap_or_default();
                let _ = writeln!(o, "\t{} = {}{meta},", v.name, v.value);
            }
            o.push_str("};\n");
        }
        o
    }

    /// Includes and forward declarations for the classes `tys` refer to.
    fn class_refs<'t>(
        &self,
        tys: impl Iterator<Item = &'t Ty>,
        includes: &mut BTreeSet<String>,
        forward: &mut BTreeSet<String>,
    ) {
        for ty in tys {
            if let Some(c) = ty.class() {
                let r = self.class(c);
                match (&r.include, r.generated) {
                    (Some(i), false) => {
                        includes.insert(i.clone());
                    }
                    _ => {
                        forward.insert(r.cpp.clone());
                    }
                }
            }
        }
    }

    fn module_header(&self) -> String {
        let m = self.module;
        let mut o = self.banner();
        o.push_str(
            "\n#pragma once\n\n#include \"CoreMinimal.h\"\n#include \"Ferreal/FerrealRuntime.h\"\n",
        );
        if !self.model.enums.is_empty() {
            let _ = writeln!(o, "#include \"Ferreal/{}.h\"", self.enums_header());
        }
        let mut includes = BTreeSet::new();
        let mut forward = BTreeSet::new();
        let tys = self
            .model
            .exports
            .iter()
            .flat_map(|e| e.item.params.iter().map(|p| &p.ty).chain([&e.item.ret]));
        self.class_refs(tys, &mut includes, &mut forward);
        for i in &includes {
            let _ = writeln!(o, "#include \"{i}\"");
        }
        for f in &forward {
            let _ = writeln!(o, "class {f};");
        }
        let _ = writeln!(o, "\nnamespace Ferreal::{m}\n{{");
        let _ = writeln!(
            o,
            "\t/** Hands Rust its tables of C++ functions. The generated classes call it before they call Rust; call it before calling exported functions from elsewhere (the module's StartupModule, say). */\n\t{} void Register();",
            self.api
        );
        for e in &self.model.exports {
            let e = &e.item;
            o.push('\n');
            doc_comment(&mut o, "\t", &e.doc);
            let _ = writeln!(
                o,
                "\t{} {} {}({});",
                self.api,
                self.value_type(&e.ret),
                e.cpp_name,
                self.params(&e.params, Site::Plain)
            );
        }
        o.push_str("}\n");
        o
    }

    fn params(&self, params: &[Param], site: Site) -> String {
        params
            .iter()
            .map(|p| format!("{} In{}", self.param_type(&p.ty, site), p.cpp_name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn args_to_rust(&self, params: &[Param]) -> Vec<String> {
        params
            .iter()
            .map(|p| self.to_rust(&p.ty, &format!("In{}", p.cpp_name)))
            .collect()
    }

    fn impl_of(&self, class: &str) -> Vec<&'a Method> {
        self.model
            .impls
            .iter()
            .filter(|i| i.item.class == class)
            .flat_map(|i| i.item.methods.iter())
            .collect()
    }

    fn on_changed(&self, class: &str) -> Option<&'a Method> {
        self.impl_of(class)
            .into_iter()
            .find(|m| m.kind == MethodKind::OnChanged)
    }

    fn class_header(&self, c: &Class) -> Result<String, String> {
        let methods = self.impl_of(&c.name);
        let mut o = self.banner();
        o.push_str("\n#pragma once\n\n#include \"CoreMinimal.h\"\n");
        for i in &c.includes {
            let _ = writeln!(o, "#include \"{i}\"");
        }
        o.push_str("#include \"Ferreal/FerrealRuntime.h\"\n");
        if !self.model.enums.is_empty() {
            let _ = writeln!(o, "#include \"Ferreal/{}.h\"", self.enums_header());
        }
        let mut includes = BTreeSet::new();
        let mut forward = BTreeSet::new();
        let tys = c.properties.iter().map(|p| &p.ty).chain(
            methods
                .iter()
                .flat_map(|m| m.params.iter().map(|p| &p.ty).chain([&m.ret])),
        );
        self.class_refs(tys, &mut includes, &mut forward);
        for i in &includes {
            let _ = writeln!(o, "#include \"{i}\"");
        }
        let _ = writeln!(
            o,
            "#include \"{}.generated.h\"\n",
            header_name(&c.cpp_name).trim_end_matches(".h")
        );
        for f in forward.iter().filter(|f| **f != c.cpp_name) {
            let _ = writeln!(o, "class {f};");
        }
        if !forward.is_empty() {
            o.push('\n');
        }
        doc_comment(&mut o, "", &c.doc);
        let _ = writeln!(o, "UCLASS({})", c.specifiers.join(", "));
        let _ = writeln!(
            o,
            "class {} {} : public {}\n{{\n\tGENERATED_BODY()\n\npublic:",
            self.api, c.cpp_name, c.parent
        );
        let _ = writeln!(
            o,
            "\t{}(const FObjectInitializer& ObjectInitializer);",
            c.cpp_name
        );
        for p in &c.properties {
            o.push('\n');
            doc_comment(&mut o, "\t", &p.doc);
            let _ = writeln!(
                o,
                "\tUPROPERTY({})\n\t{} {};",
                p.specifiers.join(", "),
                self.property_type(&p.ty),
                p.cpp_name
            );
        }
        let changed = self.on_changed(&c.name);
        for p in c.properties.iter().filter(|p| p.setter) {
            let _ = writeln!(
                o,
                "\n\t/** Sets {} and notifies the Rust class. */\n\tUFUNCTION(BlueprintSetter)\n\tvoid {}({} InValue);",
                p.cpp_name,
                p.setter_name(),
                self.param_type(&p.ty, Site::Reflected)
            );
        }
        for m in &methods {
            match &m.kind {
                MethodKind::Function { specifiers } => {
                    o.push('\n');
                    doc_comment(&mut o, "\t", &m.doc);
                    let stat = if m.receiver == Receiver::None {
                        "static "
                    } else {
                        ""
                    };
                    let cnst = if m.receiver == Receiver::Ref {
                        " const"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        o,
                        "\tUFUNCTION({})\n\t{stat}{} {}({}){cnst};",
                        specifiers.join(", "),
                        self.value_type(&m.ret),
                        m.cpp_name,
                        self.params(&m.params, Site::Reflected)
                    );
                }
                MethodKind::Override { is_const, .. } => {
                    o.push('\n');
                    doc_comment(&mut o, "\t", &m.doc);
                    let cnst = if *is_const { " const" } else { "" };
                    let _ = writeln!(
                        o,
                        "\tvirtual {} {}({}){cnst} override;",
                        self.value_type(&m.ret),
                        m.cpp_name,
                        self.params(&m.params, Site::Reflected)
                    );
                }
                MethodKind::OnChanged | MethodKind::Init => {}
            }
        }
        o.push_str("\n\tvirtual void BeginDestroy() override;\n");
        if changed.is_some() {
            o.push_str("#if WITH_EDITOR\n\tvirtual void PostEditChangeProperty(FPropertyChangedEvent& PropertyChangedEvent) override;\n#endif\n");
        }
        o.push_str(
            "\n\t/** The Rust state of this object, created on first use (null if Rust failed to create it). */\n\tvoid* FerrealState() const;\n",
        );
        o.push_str("\nprivate:\n\tmutable void* FerrealStatePtr = nullptr;\n");
        if changed.is_some() {
            o.push_str("\tvoid FerrealChanged(uint32 Index);\n");
        }
        o.push_str("};\n");
        Ok(o)
    }

    fn implementation(&self) -> Result<String, String> {
        let m = self.module;
        let s = &self.symbols;
        let mut o = self.banner();
        let _ = writeln!(o, "\n#include \"Ferreal/Ferreal{m}.h\"");
        for c in &self.model.classes {
            let _ = writeln!(o, "#include \"Ferreal/{}\"", header_name(&c.class.cpp_name));
        }
        let mut includes = BTreeSet::new();
        for e in &self.model.engine {
            if let Some(i) = &e.decl.include {
                includes.insert(i.clone());
            }
        }
        for b in &self.model.bases {
            includes.extend(b.item.includes.iter().cloned());
        }
        // `cpp!` blocks: the headers of the classes they pass, and those asked for.
        for b in &self.model.cpp_blocks {
            let types = b.block.captures.iter().map(|c| &c.param.ty);
            for t in types.chain([&b.block.ret]) {
                if let Some(c) = t.class()
                    && let Some(i) = &self.class(c).include
                {
                    includes.insert(i.clone());
                }
            }
        }
        includes.extend(self.model.cpp_includes.iter().map(|i| i.item.clone()));
        // The structs the engine bindings copy.
        includes.extend(self.engine.headers().map(str::to_string));
        for i in &includes {
            let _ = writeln!(o, "#include \"{i}\"");
        }
        if !self.engine.is_empty() {
            o.push_str("#include \"FerrealEngine.h\"\n");
        }
        let _ = writeln!(
            o,
            "\nDEFINE_LOG_CATEGORY_STATIC(LogFerreal{m}, Log, All);\n"
        );

        // Tables Rust receives.
        let _ = writeln!(o, "namespace Ferreal::{m}::Detail\n{{");
        o.push_str("\tstruct FModuleTable\n\t{\n\t\tvoid (*Log)(uint8 Level, Ferreal::FStr8 Message);\n\t};\n");
        for c in &self.model.classes {
            let c = &c.class;
            let _ = writeln!(o, "\n\tstruct FTable_{}\n\t{{", c.name);
            for p in &c.properties {
                let _ = writeln!(
                    o,
                    "\t\t{} (*Get_{})(void* Object);",
                    p.ty.cpp_ffi(true),
                    p.cpp_name
                );
                let _ = writeln!(
                    o,
                    "\t\tvoid (*Set_{})(void* Object, {} Value);",
                    p.cpp_name,
                    p.ty.cpp_ffi(false)
                );
            }
            o.push_str("\t};\n");
        }
        for b in &self.model.bases {
            let b = &b.item;
            let _ = writeln!(o, "\n\tstruct FBaseTable_{}\n\t{{", b.name);
            for bm in &b.methods {
                let params: Vec<String> = std::iter::once("void* Object".to_string())
                    .chain(
                        bm.params
                            .iter()
                            .map(|p| format!("{} In{}", p.ty.cpp_ffi(false), p.cpp_name)),
                    )
                    .collect();
                let _ = writeln!(
                    o,
                    "\t\t{} (*{})({});",
                    bm.ret.cpp_ffi(true),
                    bm.cpp_name,
                    params.join(", ")
                );
            }
            o.push_str("\t};\n");
        }
        o.push_str(&self.engine.cpp_structs());
        if !self.engine.is_empty() {
            o.push_str(&self.engine.cpp_table_type());
        }
        if !self.model.cpp_blocks.is_empty() {
            o.push_str(
                "\n\tstruct FCppEntry\n\t{\n\t\tuint64 Key;\n\t\tconst void* Function;\n\t};\n",
            );
        }
        o.push_str("}\n\n");

        // The Rust functions.
        o.push_str("extern \"C\"\n{\n");
        let _ = writeln!(
            o,
            "\tvoid {}(const Ferreal::{m}::Detail::FModuleTable* Table);",
            s.init()
        );
        for c in &self.model.classes {
            let c = &c.class;
            let _ = writeln!(o, "\tvoid* {}(void* Object);", s.new_state(&c.name));
            let _ = writeln!(o, "\tvoid {}(void* State);", s.drop_state(&c.name));
            let _ = writeln!(
                o,
                "\tvoid {}(const Ferreal::{m}::Detail::FTable_{}* Table);",
                s.register_class(&c.name),
                c.name
            );
            for meth in self.impl_of(&c.name) {
                match meth.kind {
                    MethodKind::Init => {}
                    MethodKind::OnChanged => {
                        let _ = writeln!(
                            o,
                            "\tvoid {}(void* State, uint32 Index);",
                            s.method(&c.name, &meth.name)
                        );
                    }
                    _ => {
                        let mut params: Vec<String> = Vec::new();
                        if meth.receiver != Receiver::None {
                            params.push("void* State".into());
                        }
                        params.extend(
                            meth.params
                                .iter()
                                .map(|p| format!("{} In{}", p.ty.cpp_ffi(true), p.cpp_name)),
                        );
                        let _ = writeln!(
                            o,
                            "\t{} {}({});",
                            meth.ret.cpp_ffi(false),
                            s.method(&c.name, &meth.name),
                            params.join(", ")
                        );
                    }
                }
            }
        }
        for b in &self.model.bases {
            let b = &b.item;
            let _ = writeln!(
                o,
                "\tvoid {}(const Ferreal::{m}::Detail::FBaseTable_{}* Table);",
                s.register_base(&b.name),
                b.name
            );
        }
        if !self.engine.is_empty() {
            let _ = writeln!(
                o,
                "\tvoid {}(const Ferreal::{m}::Detail::FEngineTable* Table);",
                s.register_engine()
            );
        }
        if !self.model.cpp_blocks.is_empty() {
            let _ = writeln!(
                o,
                "\tvoid {}(const Ferreal::{m}::Detail::FCppEntry* Entries, size_t Count);",
                s.register_cpp()
            );
        }
        for e in &self.model.exports {
            let e = &e.item;
            let params: Vec<String> = e
                .params
                .iter()
                .map(|p| format!("{} In{}", p.ty.cpp_ffi(true), p.cpp_name))
                .collect();
            let _ = writeln!(
                o,
                "\t{} {}({});",
                e.ret.cpp_ffi(false),
                s.export(&e.name),
                params.join(", ")
            );
        }
        o.push_str("}\n\n");

        // The C++ functions Rust calls.
        let _ = writeln!(o, "namespace Ferreal::{m}::Detail\n{{");
        let _ = writeln!(
            o,
            "\tstatic void Log(uint8 Level, Ferreal::FStr8 Message)\n\t{{\n\t\tconst FString Text = Ferreal::ToFString(Message);\n\t\tswitch (Level)\n\t\t{{\n\t\tcase 0: UE_LOG(LogFerreal{m}, Log, TEXT(\"%s\"), *Text); break;\n\t\tcase 1: UE_LOG(LogFerreal{m}, Warning, TEXT(\"%s\"), *Text); break;\n\t\tdefault: UE_LOG(LogFerreal{m}, Error, TEXT(\"%s\"), *Text); break;\n\t\t}}\n\t}}"
        );
        for c in &self.model.classes {
            let c = &c.class;
            for p in &c.properties {
                let get = self.to_rust(
                    &p.ty,
                    &format!(
                        "static_cast<{}*>(static_cast<UObject*>(Object))->{}",
                        c.cpp_name, p.cpp_name
                    ),
                );
                let get = if matches!(p.ty, Ty::ObjArray { .. }) {
                    format!(
                        "Ferreal::KeepObjs(static_cast<{}*>(static_cast<UObject*>(Object))->{})",
                        c.cpp_name, p.cpp_name
                    )
                } else {
                    get
                };
                let _ = writeln!(
                    o,
                    "\n\tstatic {} Get_{}_{}(void* Object)\n\t{{\n\t\treturn {get};\n\t}}",
                    p.ty.cpp_ffi(true),
                    c.name,
                    p.cpp_name
                );
                let _ = writeln!(
                    o,
                    "\n\tstatic void Set_{}_{}(void* Object, {} Value)\n\t{{\n\t\tstatic_cast<{}*>(static_cast<UObject*>(Object))->{} = {};\n\t}}",
                    c.name,
                    p.cpp_name,
                    p.ty.cpp_ffi(false),
                    c.cpp_name,
                    p.cpp_name,
                    self.rust_to_cpp(&p.ty, "Value", true)
                );
            }
            let entries: Vec<String> = c
                .properties
                .iter()
                .flat_map(|p| {
                    [
                        format!("&Get_{}_{}", c.name, p.cpp_name),
                        format!("&Set_{}_{}", c.name, p.cpp_name),
                    ]
                })
                .collect();
            let _ = writeln!(
                o,
                "\n\tstatic const FTable_{0} Table_{0} = {{{1}}};",
                c.name,
                entries.join(", ")
            );
        }
        for b in &self.model.bases {
            self.base_thunks(&mut o, &b.item);
        }
        if !self.engine.is_empty() {
            o.push_str(&self.engine.cpp_thunks());
        }
        o.push_str("}\n\n");

        // `cpp!` blocks, in a namespace of their own (their C++ sees the global names).
        if !self.model.cpp_blocks.is_empty() {
            self.cpp_blocks(&mut o);
        }

        // Registration.
        let _ = writeln!(
            o,
            "void Ferreal::{m}::Register()\n{{\n\tstatic const bool bRegistered = []\n\t{{"
        );
        let _ = writeln!(
            o,
            "\t\tstatic const Detail::FModuleTable ModuleTable = {{&Detail::Log}};\n\t\t{}(&ModuleTable);",
            s.init()
        );
        for c in &self.model.classes {
            let _ = writeln!(
                o,
                "\t\t{}(&Detail::Table_{});",
                s.register_class(&c.class.name),
                c.class.name
            );
        }
        for b in &self.model.bases {
            let _ = writeln!(
                o,
                "\t\t{}(&Detail::BaseTable_{});",
                s.register_base(&b.item.name),
                b.item.name
            );
        }
        if !self.engine.is_empty() {
            let _ = writeln!(o, "\t\t{}(&Detail::EngineTable);", s.register_engine());
        }
        if !self.model.cpp_blocks.is_empty() {
            let _ = writeln!(
                o,
                "\t\t{}(FerrealCpp{m}::Entries, sizeof(FerrealCpp{m}::Entries) / sizeof(FerrealCpp{m}::Entries[0]));",
                s.register_cpp()
            );
        }
        o.push_str("\t\treturn true;\n\t}();\n\t(void)bRegistered;\n}\n");

        // Exported functions.
        for e in &self.model.exports {
            self.export_body(&mut o, &e.item);
        }
        // Classes.
        for c in &self.model.classes {
            self.class_body(&mut o, &c.class)?;
        }
        Ok(o)
    }

    fn base_thunks(&self, o: &mut String, b: &Base) {
        for bm in &b.methods {
            let params: Vec<String> = std::iter::once("void* Object".to_string())
                .chain(
                    bm.params
                        .iter()
                        .map(|p| format!("{} In{}", p.ty.cpp_ffi(false), p.cpp_name)),
                )
                .collect();
            let args: Vec<String> = bm
                .params
                .iter()
                .map(|p| self.rust_to_cpp(&p.ty, &format!("In{}", p.cpp_name), false))
                .collect();
            let call = format!(
                "static_cast<{}*>(static_cast<UObject*>(Object))->{}({})",
                b.cpp_class,
                bm.cpp_name,
                args.join(", ")
            );
            let body = if bm.ret == Ty::Unit {
                format!("{call};")
            } else {
                format!("return {};", self.keep_for_rust(&bm.ret, &call))
            };
            let _ = writeln!(
                o,
                "\n\tstatic {} Base_{}_{}({})\n\t{{\n\t\t{body}\n\t}}",
                bm.ret.cpp_ffi(true),
                b.name,
                bm.cpp_name,
                params.join(", ")
            );
        }
        let entries: Vec<String> = b
            .methods
            .iter()
            .map(|bm| format!("&Base_{}_{}", b.name, bm.cpp_name))
            .collect();
        let _ = writeln!(
            o,
            "\n\tstatic const FBaseTable_{0} BaseTable_{0} = {{{1}}};",
            b.name,
            entries.join(", ")
        );
    }

    /// A C++ function per `cpp!` block (identical blocks share one), and the table of them,
    /// sorted by key, that Rust looks them up in.
    fn cpp_blocks(&self, o: &mut String) {
        let m = self.module;
        let mut blocks = std::collections::BTreeMap::new();
        for b in &self.model.cpp_blocks {
            blocks.entry(b.key).or_insert(b);
        }
        // Types passed by name that are not the crate's `#[uenum]`s: engine structs and enums,
        // math types (their conversions are in `Detail`).
        let scope = format!("Ferreal::{m}::Detail::");
        let named = |t: &Ty| match t {
            Ty::Enum(n) if !self.enums.contains_key(n.as_str()) => self.engine.named(n),
            _ => None,
        };
        let _ = writeln!(o, "namespace FerrealCpp{m}\n{{");
        for (key, b) in &blocks {
            let name = ferreal_model::cpp::key_name(*key);
            let block = &b.block;
            let params: Vec<String> = block
                .captures
                .iter()
                .map(|c| {
                    let carrier = match named(&c.param.ty) {
                        Some(t) => t.cpp_carrier(false, &scope),
                        None => c.param.ty.cpp_ffi(false),
                    };
                    format!("{carrier} FerrealIn_{}", c.param.cpp_name)
                })
                .collect();
            let ret = named(&block.ret);
            let _ = writeln!(
                o,
                "\t// cpp! at {}\n\tstatic {} Cpp_{name}({})\n\t{{",
                b.location(&self.model.dir),
                ret.as_ref()
                    .map_or_else(|| block.ret.cpp_ffi(true), |t| t.cpp_carrier(true, &scope)),
                params.join(", ")
            );
            for c in &block.captures {
                let n = &c.param.cpp_name;
                let input = format!("FerrealIn_{n}");
                let value = match named(&c.param.ty) {
                    Some(t) => t.from_carrier(&input, &scope),
                    None => self.rust_to_cpp(&c.param.ty, &input, false),
                };
                let _ = writeln!(o, "\t\tauto {n} = {value};\n\t\t(void){n};");
            }
            let _ = writeln!(
                o,
                "\t\tauto Body = [&]() -> {}\n\t\t{{{}}};",
                ret.as_ref()
                    .map_or_else(|| self.value_type(&block.ret), |t| t.cpp_value()),
                reindent(&b.text, "\t\t\t", "\t\t")
            );
            if block.ret == Ty::Unit {
                o.push_str("\t\tBody();\n\t}\n\n");
            } else {
                let value = match &ret {
                    Some(t) => t.to_carrier("Body()", &scope),
                    None => self.keep_for_rust(&block.ret, "Body()"),
                };
                let _ = writeln!(o, "\t\treturn {value};\n\t}}\n");
            }
        }
        let entries: Vec<String> = blocks
            .keys()
            .map(|k| {
                let name = ferreal_model::cpp::key_name(*k);
                format!("{{0x{name}ull, reinterpret_cast<const void*>(&Cpp_{name})}}")
            })
            .collect();
        let _ = writeln!(
            o,
            "\tstatic const Ferreal::{m}::Detail::FCppEntry Entries[] = {{\n\t\t{}\n\t}};\n}}\n",
            entries.join(",\n\t\t")
        );
    }

    fn export_body(&self, o: &mut String, e: &Export) {
        let m = self.module;
        let call = format!(
            "{}({})",
            self.symbols.export(&e.name),
            self.args_to_rust(&e.params).join(", ")
        );
        let body = if e.ret == Ty::Unit {
            format!("{call};")
        } else {
            format!("return {};", self.returned_from_rust(&e.ret, &call))
        };
        let _ = writeln!(
            o,
            "\n{} Ferreal::{m}::{}({})\n{{\n\tRegister();\n\t{body}\n}}",
            self.value_type(&e.ret),
            e.cpp_name,
            self.params(&e.params, Site::Plain)
        );
    }

    fn class_body(&self, o: &mut String, c: &Class) -> Result<(), String> {
        let m = self.module;
        let s = &self.symbols;
        let cpp = &c.cpp_name;
        let _ = writeln!(o, "\n// {cpp}\n");
        let _ = writeln!(
            o,
            "{cpp}::{cpp}(const FObjectInitializer& ObjectInitializer)\n\t: Super(ObjectInitializer)\n{{"
        );
        for p in &c.properties {
            if let Some(d) = &p.default {
                let _ = writeln!(o, "\t{} = {};", p.cpp_name, self.default_value(p, d)?);
            }
        }
        o.push_str("}\n");
        let _ = writeln!(
            o,
            "\nvoid* {cpp}::FerrealState() const\n{{\n\tif (!FerrealStatePtr)\n\t{{\n\t\tFerreal::{m}::Register();\n\t\tFerrealStatePtr = {}(static_cast<UObject*>(const_cast<{cpp}*>(this)));\n\t}}\n\treturn FerrealStatePtr;\n}}",
            s.new_state(&c.name)
        );
        let _ = writeln!(
            o,
            "\nvoid {cpp}::BeginDestroy()\n{{\n\tif (FerrealStatePtr)\n\t{{\n\t\t{}(FerrealStatePtr);\n\t\tFerrealStatePtr = nullptr;\n\t}}\n\tSuper::BeginDestroy();\n}}",
            s.drop_state(&c.name)
        );
        let changed = self.on_changed(&c.name);
        if let Some(ch) = changed {
            let _ = writeln!(
                o,
                "\nvoid {cpp}::FerrealChanged(uint32 Index)\n{{\n\tif (void* State = FerrealState())\n\t{{\n\t\t{}(State, Index);\n\t}}\n}}",
                s.method(&c.name, &ch.name)
            );
            let _ = writeln!(
                o,
                "\n#if WITH_EDITOR\nvoid {cpp}::PostEditChangeProperty(FPropertyChangedEvent& PropertyChangedEvent)\n{{\n\tSuper::PostEditChangeProperty(PropertyChangedEvent);\n\tconst FName FerrealChangedName = PropertyChangedEvent.GetMemberPropertyName();"
            );
            for (i, p) in c.properties.iter().enumerate() {
                let kw = if i == 0 { "if" } else { "else if" };
                let _ = writeln!(
                    o,
                    "\t{kw} (FerrealChangedName == GET_MEMBER_NAME_CHECKED({cpp}, {}))\n\t{{\n\t\tFerrealChanged({i});\n\t}}",
                    p.cpp_name
                );
            }
            o.push_str("}\n#endif\n");
        }
        for (i, p) in c.properties.iter().enumerate().filter(|(_, p)| p.setter) {
            let notify = if changed.is_some() {
                format!("\n\tFerrealChanged({i});")
            } else {
                String::new()
            };
            let _ = writeln!(
                o,
                "\nvoid {cpp}::{}({} InValue)\n{{\n\t{} = InValue;{notify}\n}}",
                p.setter_name(),
                self.param_type(&p.ty, Site::Reflected),
                p.cpp_name
            );
        }
        for meth in self.impl_of(&c.name) {
            self.method_body(o, c, meth);
        }
        Ok(())
    }

    fn method_body(&self, o: &mut String, c: &Class, meth: &Method) {
        let cpp = &c.cpp_name;
        let sym = self.symbols.method(&c.name, &meth.name);
        let (call_super, cnst) = match &meth.kind {
            MethodKind::Function { .. } => (CallSuper::Never, meth.receiver == Receiver::Ref),
            MethodKind::Override {
                call_super,
                is_const,
            } => (*call_super, *is_const),
            MethodKind::OnChanged | MethodKind::Init => return,
        };
        let cnst = if cnst { " const" } else { "" };
        let ret_ty = self.value_type(&meth.ret);
        let params = self.params(&meth.params, Site::Reflected);
        let super_call = format!(
            "Super::{}({});",
            meth.cpp_name,
            meth.params
                .iter()
                .map(|p| format!("In{}", p.cpp_name))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut body = String::new();
        if call_super == CallSuper::Before {
            let _ = writeln!(body, "\t{super_call}");
        }
        let mut args = self.args_to_rust(&meth.params);
        if meth.receiver == Receiver::None {
            let call = format!("{sym}({})", args.join(", "));
            if meth.ret == Ty::Unit {
                let _ = writeln!(body, "\tFerreal::{}::Register();\n\t{call};", self.module);
            } else {
                let _ = writeln!(
                    body,
                    "\tFerreal::{}::Register();\n\treturn {};",
                    self.module,
                    self.returned_from_rust(&meth.ret, &call)
                );
            }
        } else {
            args.insert(0, "State".into());
            let call = format!("{sym}({})", args.join(", "));
            let fallback = self.fallback(&meth.ret);
            let ret = if meth.ret == Ty::Unit {
                "return;".to_string()
            } else {
                format!("return {fallback};")
            };
            let _ = writeln!(
                body,
                "\tvoid* State = FerrealState();\n\tif (!State)\n\t{{\n\t\t{ret}\n\t}}"
            );
            if meth.ret == Ty::Unit {
                let _ = writeln!(body, "\t{call};");
            } else {
                let _ = writeln!(
                    body,
                    "\treturn {};",
                    self.returned_from_rust(&meth.ret, &call)
                );
            }
        }
        if call_super == CallSuper::After && meth.ret == Ty::Unit {
            let _ = writeln!(body, "\t{super_call}");
        }
        let _ = writeln!(
            o,
            "\n{ret_ty} {cpp}::{}({params}){cnst}\n{{\n{body}}}",
            meth.cpp_name
        );
    }
}

/// Checks that the C++ the model asks for can be written: referenced classes and enums are
/// declared, nothing is declared twice.
pub fn check(model: &Collected, engine: &Bindings) -> Vec<String> {
    let mut errors = Vec::new();
    let mut classes: HashMap<String, &str> = HashMap::new();
    let engine_markers: BTreeSet<&str> = engine.marker_classes().map(|(n, _, _)| n).collect();
    for e in &model.engine {
        if classes
            .insert(e.decl.name.to_string(), "engine_class!")
            .is_some()
        {
            errors.push(format!("{}: {} is declared twice", e.at, e.decl.name));
        }
    }
    for c in &model.classes {
        if classes.insert(c.class.name.clone(), "#[uclass]").is_some() {
            errors.push(format!("{}: {} is declared twice", c.at, c.class.name));
        }
    }
    let enums: BTreeSet<&str> = model.enums.iter().map(|e| e.item.name.as_str()).collect();
    let bases: BTreeSet<&str> = model.bases.iter().map(|b| b.item.name.as_str()).collect();
    let check_ty = |ty: &Ty, at: &str, errors: &mut Vec<String>| {
        if let Some(c) = ty.class()
            && !classes.contains_key(c)
            && !engine_markers.contains(c)
        {
            errors.push(format!(
                "{at}: Obj<{c}>: declare {c} with engine_class! or #[uclass], or name its engine class in ferreal::engine!"
            ));
        }
        if let Ty::Enum(e) = ty
            && !enums.contains(e.as_str())
        {
            errors.push(format!(
                "{at}: {e} is not a #[uenum] (or a type Ferreal passes)"
            ));
        }
    };
    for c in &model.classes {
        for p in &c.class.properties {
            check_ty(&p.ty, &c.at, &mut errors);
        }
        for b in &c.class.bases {
            let n = ferreal_model::path_name(b);
            if !bases.contains(n.as_str()) {
                errors.push(format!("{}: {n} is not a #[cpp_base] trait", c.at));
            }
        }
    }
    for i in &model.impls {
        if !model.classes.iter().any(|c| c.class.name == i.item.class) {
            errors.push(format!(
                "{}: #[uclass_impl] for {}, which is not a #[uclass]",
                i.at, i.item.class
            ));
        }
        for m in &i.item.methods {
            for t in m.params.iter().map(|p| &p.ty).chain([&m.ret]) {
                check_ty(t, &i.at, &mut errors);
            }
        }
    }
    for b in &model.bases {
        for m in &b.item.methods {
            for t in m.params.iter().map(|p| &p.ty).chain([&m.ret]) {
                check_ty(t, &b.at, &mut errors);
            }
        }
    }
    for e in &model.exports {
        for t in e.item.params.iter().map(|p| &p.ty).chain([&e.item.ret]) {
            check_ty(t, &e.at, &mut errors);
        }
    }
    for b in &model.cpp_blocks {
        let captures = b.block.captures.iter().map(|c| &c.param.ty);
        for t in captures.chain([&b.block.ret]) {
            // Types by name may be the engine's (resolved with the bindings).
            if let Ty::Enum(n) = t
                && engine.named(n).is_some()
            {
                continue;
            }
            check_ty(t, &b.at, &mut errors);
        }
    }
    for e in &model.engine {
        for b in &e.decl.bases {
            let n = ferreal_model::path_name(b);
            if !bases.contains(n.as_str()) {
                errors.push(format!("{}: {n} is not a #[cpp_base] trait", e.at));
            }
        }
    }
    errors
}
