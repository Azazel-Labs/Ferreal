//! The build-script half of Ferreal: reads a crate's Unreal-facing items (`#[uclass]`,
//! `#[uclass_impl]`, `#[uenum]`, `#[cpp_base]`, `engine_class!`, `#[export]`, and the C++ of
//! its `cpp!` blocks) from its sources and writes their C++ into an Unreal module.
//!
//! ```no_run
//! // build.rs
//! ferreal_build::Module::new("Glyphux")
//!     .output("../Source/Glyphux")
//!     .run();
//! ```
//!
//! It writes, under the module directory:
//!
//! | File | |
//! |---|---|
//! | `Public/Ferreal/FerrealRuntime.h` | The glue's types and conversions. |
//! | `Public/Ferreal/Ferreal<Module>.h` | `Ferreal::<Module>::Register()` and the exported functions. |
//! | `Public/Ferreal/Ferreal<Module>Enums.h` | The `UENUM`s. |
//! | `Public/Ferreal/<Class>.h` | A `UCLASS` per `#[uclass]` (`UGlyphuxText` in `GlyphuxText.h`). |
//! | `Private/Ferreal/Ferreal<Module>.cpp` | Everything they do, calling into Rust. |
//!
//! Only changed files are written, and files of earlier builds that are no longer generated
//! are removed from those two `Ferreal` folders (nothing else is touched). It also passes the
//! module name to the macros (`FERREAL_MODULE`).
//!
//! The engine classes `ferreal::engine!` names are bound from Unreal's reflection data (see
//! [`engine`]): their Rust goes to `OUT_DIR/ferreal_engine.rs` (the crate's `engine` module),
//! their C++ thunks into the module's `.cpp` (and `Private/Ferreal/FerrealEngine.h`), and the
//! classes used into the lock file `ferreal-engine.json` next to `Cargo.toml`.

mod cpp;
pub mod engine;

use std::path::{Path, PathBuf};

use ferreal_model::attr::args_of;
use ferreal_model::{
    Base, Class, ClassImpl, CppBlock, CppBody, EngineClass, EngineList, Enum, Export,
};
use syn::Item;
use syn::visit::Visit;

/// An item and where it is (`file:line`).
#[derive(Clone, Debug)]
pub struct At<T> {
    pub item: T,
    pub at: String,
}

#[derive(Clone, Debug)]
pub struct ClassAt {
    pub class: Class,
    pub at: String,
    /// Its path from the crate root (`crate::text::Label`).
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct EngineAt {
    pub decl: EngineClass,
    pub at: String,
    /// The marker's path from the crate root.
    pub path: String,
}

/// Everything the crate exposes.
#[derive(Clone, Debug, Default)]
pub struct Collected {
    pub classes: Vec<ClassAt>,
    pub impls: Vec<At<ClassImpl>>,
    pub enums: Vec<At<Enum>>,
    pub bases: Vec<At<Base>>,
    pub engine: Vec<EngineAt>,
    /// `ferreal::engine!` lists.
    pub engine_lists: Vec<At<EngineList>>,
    pub exports: Vec<At<Export>>,
    /// `cpp!` blocks, in the order found.
    pub cpp_blocks: Vec<CppAt>,
    /// `ferreal::cpp_include!` headers.
    pub cpp_includes: Vec<At<String>>,
    /// The crate's directory, which the generated C++ gives locations relative to.
    pub dir: PathBuf,
    /// The source files read.
    pub files: Vec<PathBuf>,
}

/// A `cpp!` block found in the sources.
#[derive(Clone, Debug)]
pub struct CppAt {
    pub block: CppBlock,
    /// Its key (`ferreal_model::cpp::key`), which the macro computes too.
    pub key: u64,
    pub at: String,
    pub file: PathBuf,
    pub line: usize,
    /// The C++, as written.
    pub text: String,
}

impl CppAt {
    /// `file:line`, the file relative to `dir` (the crate's) when it is under it, with `/`
    /// separators: what the generated C++ notes, the same on every machine.
    pub fn location(&self, dir: &Path) -> String {
        let file = self.file.strip_prefix(dir).unwrap_or(&self.file);
        let file: Vec<String> = file
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        format!("{}:{}", file.join("/"), self.line)
    }
}

/// What went wrong, with `file:line:column` locations.
#[derive(Debug)]
pub struct Error(pub Vec<String>);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.0 {
            writeln!(f, "ferreal: {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

fn located(file: &Path, e: &syn::Error) -> String {
    let mut out = Vec::new();
    for e in e.clone() {
        let start = e.span().start();
        out.push(format!(
            "{}:{}:{}: {e}",
            file.display(),
            start.line,
            start.column + 1
        ));
    }
    out.join("\n")
}

fn at(file: &Path, span: proc_macro2::Span) -> String {
    let start = span.start();
    format!("{}:{}", file.display(), start.line)
}

/// A file to read: its path, whether its submodules' files sit next to it (`lib.rs`,
/// `mod.rs`), and its module path from the crate root.
type Queued = (PathBuf, bool, Vec<String>);

/// Reads the crate whose root is `root` (`src/lib.rs`), following `mod` declarations.
/// [`check`] then checks what it found refers to things that exist.
pub fn collect(root: &Path) -> Result<Collected, Error> {
    let mut out = Collected::default();
    let mut errors = Vec::new();
    let mut queue: Vec<Queued> = vec![(root.to_path_buf(), true, Vec::new())];
    while let Some((file, owns_dir, mod_path)) = queue.pop() {
        let text = match std::fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{}: {e}", file.display()));
                continue;
            }
        };
        let parsed = match syn::parse_file(&text) {
            Ok(f) => f,
            Err(e) => {
                errors.push(located(&file, &e));
                continue;
            }
        };
        out.files.push(file.clone());
        CppFinder {
            file: &file,
            text: &text,
            out: &mut out,
            errors: &mut errors,
        }
        .visit_file(&parsed);
        // Where its submodules' files are: next to it for lib.rs and mod.rs, else in a folder
        // of its name.
        let dir = if owns_dir {
            file.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            file.with_extension("")
        };
        walk(
            &parsed.items,
            &file,
            &dir,
            &mod_path,
            &mut out,
            &mut errors,
            &mut queue,
        );
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(Error(errors))
    }
}

/// Checks that what `model` refers to exists: classes (declared, or engine classes the
/// `bindings` have markers of) and enums, and that nothing is declared twice.
pub fn check(model: &Collected, bindings: &engine::Bindings) -> Result<(), Error> {
    let errors = cpp::check(model, bindings);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error(errors))
    }
}

fn rust_path(mod_path: &[String], name: &str) -> String {
    let mut p = String::from("crate");
    for m in mod_path {
        p.push_str("::");
        p.push_str(m);
    }
    p.push_str("::");
    p.push_str(name);
    p
}

fn path_attr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|a| match &a.meta {
        syn::Meta::NameValue(nv) if nv.path.is_ident("path") => match &nv.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => Some(s.value()),
            _ => None,
        },
        _ => None,
    })
}

fn walk(
    items: &[Item],
    file: &Path,
    dir: &Path,
    mod_path: &[String],
    out: &mut Collected,
    errors: &mut Vec<String>,
    queue: &mut Vec<Queued>,
) {
    fn report(errors: &mut Vec<String>, file: &Path, r: syn::Result<()>) {
        if let Err(e) = r {
            errors.push(located(file, &e));
        }
    }
    for item in items {
        match item {
            Item::Mod(m) => {
                let name = m.ident.to_string();
                let mut inner_path = mod_path.to_vec();
                inner_path.push(name.clone());
                match &m.content {
                    Some((_, inner)) => walk(
                        inner,
                        file,
                        &dir.join(&name),
                        &inner_path,
                        out,
                        errors,
                        queue,
                    ),
                    None => {
                        if let Some(p) = path_attr(&m.attrs) {
                            queue.push((dir.join(p), true, inner_path));
                        } else {
                            let flat = dir.join(format!("{name}.rs"));
                            if flat.exists() {
                                queue.push((flat, false, inner_path));
                            } else {
                                queue.push((dir.join(&name).join("mod.rs"), true, inner_path));
                            }
                        }
                    }
                }
            }
            Item::Struct(s) => report(
                errors,
                file,
                (|| {
                    if let Some(args) = args_of(&s.attrs, "uclass")? {
                        out.classes.push(ClassAt {
                            class: Class::parse(&args, s)?,
                            at: at(file, s.ident.span()),
                            path: rust_path(mod_path, &s.ident.to_string()),
                        });
                    }
                    Ok(())
                })(),
            ),
            Item::Impl(i) => report(
                errors,
                file,
                (|| {
                    if args_of(&i.attrs, "uclass_impl")?.is_some() {
                        out.impls.push(At {
                            item: ClassImpl::parse(i)?,
                            at: at(file, i.impl_token.span),
                        });
                    }
                    Ok(())
                })(),
            ),
            Item::Enum(e) => report(
                errors,
                file,
                (|| {
                    if let Some(args) = args_of(&e.attrs, "uenum")? {
                        out.enums.push(At {
                            item: Enum::parse(&args, e)?,
                            at: at(file, e.ident.span()),
                        });
                    }
                    Ok(())
                })(),
            ),
            Item::Trait(t) => report(
                errors,
                file,
                (|| {
                    if let Some(args) = args_of(&t.attrs, "cpp_base")? {
                        out.bases.push(At {
                            item: Base::parse(&args, t)?,
                            at: at(file, t.ident.span()),
                        });
                    }
                    Ok(())
                })(),
            ),
            Item::Fn(f) => report(
                errors,
                file,
                (|| {
                    if let Some(args) = args_of(&f.attrs, "export")? {
                        out.exports.push(At {
                            item: Export::parse(&args, f)?,
                            at: at(file, f.sig.ident.span()),
                        });
                    }
                    Ok(())
                })(),
            ),
            Item::Macro(m)
                if m.mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "engine_class") =>
            {
                report(
                    errors,
                    file,
                    (|| {
                        let decl: EngineClass = syn::parse2(m.mac.tokens.clone())?;
                        out.engine.push(EngineAt {
                            at: at(file, decl.name.span()),
                            path: rust_path(mod_path, &decl.name.to_string()),
                            decl,
                        });
                        Ok(())
                    })(),
                );
            }
            Item::Macro(m)
                if m.mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "engine") =>
            {
                report(
                    errors,
                    file,
                    (|| {
                        let list: EngineList = syn::parse2(m.mac.tokens.clone())?;
                        out.engine_lists.push(At {
                            at: at(
                                file,
                                m.mac.path.segments.last().expect("a name").ident.span(),
                            ),
                            item: list,
                        });
                        Ok(())
                    })(),
                );
            }
            _ => {}
        }
    }
}

/// Finds the `cpp!` and `cpp_include!` invocations of a file, wherever they are (function
/// bodies included; not inside other macros' arguments, which are not parsed).
struct CppFinder<'a> {
    file: &'a Path,
    text: &'a str,
    out: &'a mut Collected,
    errors: &'a mut Vec<String>,
}

impl CppFinder<'_> {
    fn block(&mut self, m: &syn::Macro) -> syn::Result<()> {
        let block: CppBlock = syn::parse2(m.tokens.clone())?;
        let text = match &block.body {
            CppBody::Text(s) => s.value(),
            CppBody::Tokens(g) => {
                let (start, end) = (g.span_open().end(), g.span_close().start());
                match (self.offset(start), self.offset(end)) {
                    (Some(a), Some(b)) if a <= b => self.text[a..b].to_string(),
                    _ => {
                        return Err(syn::Error::new(
                            g.span(),
                            "cannot locate this cpp! block in its file",
                        ));
                    }
                }
            }
        };
        let span = m.path.segments.last().expect("a name").ident.span();
        self.out.cpp_blocks.push(CppAt {
            key: ferreal_model::cpp::key(&m.tokens),
            at: at(self.file, span),
            file: self.file.to_path_buf(),
            line: span.start().line,
            text,
            block,
        });
        Ok(())
    }

    /// The byte offset of a line (1-based) and column (in characters).
    fn offset(&self, at: proc_macro2::LineColumn) -> Option<usize> {
        let line_start = if at.line == 1 {
            0
        } else {
            self.text
                .match_indices('\n')
                .nth(at.line - 2)
                .map(|(i, _)| i + 1)?
        };
        let line = &self.text[line_start..];
        let column = line
            .char_indices()
            .nth(at.column)
            .map_or(line.len(), |(i, _)| i);
        Some(line_start + column)
    }
}

impl<'ast> Visit<'ast> for CppFinder<'_> {
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        let name = m.path.segments.last().map(|s| s.ident.to_string());
        let r = match name.as_deref() {
            Some("cpp") => self.block(m),
            Some("cpp_include") => syn::parse2::<syn::LitStr>(m.tokens.clone()).map(|s| {
                self.out.cpp_includes.push(At {
                    item: s.value(),
                    at: at(self.file, s.span()),
                });
            }),
            _ => Ok(()),
        };
        if let Err(e) = r {
            self.errors.push(located(self.file, &e));
        }
        syn::visit::visit_macro(self, m);
    }
}

/// A generated file.
pub struct GeneratedFile {
    /// Relative to the module directory.
    pub path: String,
    pub contents: String,
}

/// Generates the C++ of `model` for module `module` (`api`: its `<MODULE>_API` macro;
/// `source`: how the banner names the crate), with the thunks of the engine `bindings`.
pub fn generate(
    module: &str,
    api: &str,
    source: &str,
    model: &Collected,
    bindings: &engine::Bindings,
) -> Result<Vec<GeneratedFile>, Error> {
    let generator = cpp::Gen::new(module, api, source, model, bindings);
    generator
        .files()
        .map(|files| {
            files
                .into_iter()
                .map(|f| GeneratedFile {
                    path: f.path,
                    contents: f.contents,
                })
                .collect()
        })
        .map_err(|e| Error(vec![e]))
}

/// The configuration of a build script.
pub struct Module {
    name: String,
    root: PathBuf,
    output: Option<PathBuf>,
    api: Option<String>,
    lock: PathBuf,
}

impl Module {
    /// The Unreal module the crate belongs to (its name: `Glyphux` for `Glyphux.Build.cs`).
    pub fn new(name: &str) -> Self {
        Module {
            name: name.into(),
            root: PathBuf::from("src/lib.rs"),
            output: None,
            api: None,
            lock: PathBuf::from(engine::LOCK_FILE),
        }
    }

    /// The lock file of the engine classes used, relative to the crate (default
    /// `ferreal-engine.json`). `FERREAL_ENGINE_LOCK`, if set, overrides it.
    pub fn engine_lock(mut self, path: impl Into<PathBuf>) -> Self {
        self.lock = path.into();
        self
    }

    /// The crate root, relative to the crate (default `src/lib.rs`).
    pub fn root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }

    /// The Unreal module's directory (with its `Public` and `Private` folders), relative to
    /// the crate. Without one, nothing is written (the macros still get the module name).
    /// `FERREAL_OUTPUT`, if set, overrides it.
    pub fn output(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output = Some(dir.into());
        self
    }

    /// The module's export macro (default `<NAME>_API`).
    pub fn api(mut self, api: &str) -> Self {
        self.api = Some(api.into());
        self
    }

    /// Runs as a build script: tells cargo what to watch and passes the module name to the
    /// macros, then writes the C++. Panics with every problem found, located in the sources.
    pub fn run(self) {
        if let Err(e) = self.try_run() {
            panic!("\n{e}");
        }
    }

    /// [`Module::run`], returning the problems instead of panicking.
    pub fn try_run(self) -> Result<(), Error> {
        println!(
            "cargo:rustc-env={}={}",
            ferreal_model::MODULE_ENV,
            self.name
        );
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed={}", engine::METADATA_ENV);
        println!("cargo:rerun-if-env-changed={}", engine::LOCK_ENV);
        println!("cargo:rerun-if-env-changed={}", engine::OUTPUT_ENV);
        let crate_dir =
            std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(PathBuf::new, PathBuf::from);
        let root = crate_dir.join(&self.root);
        let mut model = collect(&root)?;
        model.dir = crate_dir.clone();
        for f in &model.files {
            println!("cargo:rerun-if-changed={}", f.display());
        }

        // The engine bindings. Without the data they need, the rest is still written (so the
        // Unreal build that exports the data can run), and the error reported after.
        let lock = match std::env::var_os(engine::LOCK_ENV).filter(|p| !p.is_empty()) {
            Some(p) => PathBuf::from(p),
            None => crate_dir.join(&self.lock),
        };
        let (bindings, engine_errors) = match engine_bindings(&model, &lock) {
            Ok(b) => (b, Vec::new()),
            Err(e) => (engine::Bindings::default(), e),
        };
        let symbols = ferreal_model::Symbols::new(&self.name);
        if let Some(out_dir) = std::env::var_os("OUT_DIR") {
            let path = PathBuf::from(out_dir).join("ferreal_engine.rs");
            write_if_changed(&path, &bindings.rust(&symbols.register_engine()))
                .map_err(|e| Error(vec![format!("{}: {e}", path.display())]))?;
        }
        // Without the bindings, items that mention engine classes cannot be written: leave
        // the C++ as the last build wrote it, and report what is missing.
        let checked = check(&model, &bindings);
        if engine_errors.is_empty() {
            checked?;
        } else if checked.is_err() {
            return Err(Error(engine_errors));
        }

        let output = std::env::var_os(engine::OUTPUT_ENV)
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| self.output.as_ref().map(|o| crate_dir.join(o)));
        if let Some(output) = &output {
            let api = self
                .api
                .clone()
                .unwrap_or_else(|| format!("{}_API", self.name.to_uppercase()));
            let source = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "the crate".into());
            let files = generate(&self.name, &api, &source, &model, &bindings)?;
            write_files(output, &files)
                .map_err(|e| Error(vec![format!("{}: {e}", output.display())]))?;
        }
        if engine_errors.is_empty() {
            Ok(())
        } else {
            Err(Error(engine_errors))
        }
    }
}

/// The engine bindings of `model`: from the exporter's data when the Unreal build passes it
/// (`FERREAL_ENGINE_METADATA`), whose part used is then kept in the lock file, else from the
/// lock file.
fn engine_bindings(model: &Collected, lock: &Path) -> Result<engine::Bindings, Vec<String>> {
    let requests: Vec<engine::Request> = model
        .engine_lists
        .iter()
        .flat_map(|l| {
            l.item.classes.iter().map(|c| engine::Request {
                cpp: c.to_string(),
                at: l.at.clone(),
            })
        })
        .collect();
    let handles: Vec<engine::Handle> = model
        .classes
        .iter()
        .map(|c| engine::Handle {
            rust_path: c.path.clone(),
            cpp: c.class.cpp_name.clone(),
            own: true,
        })
        .chain(model.engine.iter().map(|e| engine::Handle {
            rust_path: e.path.clone(),
            cpp: e.decl.cpp_class.clone(),
            own: false,
        }))
        .collect();
    let exported = std::env::var_os(engine::METADATA_ENV)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    let source = match &exported {
        Some(p) if p.exists() => {
            println!("cargo:rerun-if-changed={}", p.display());
            engine::Source::Exported(engine::Metadata::load(p).map_err(|e| vec![e])?)
        }
        _ if lock.exists() => {
            println!("cargo:rerun-if-changed={}", lock.display());
            engine::Source::Lock(engine::Metadata::load(lock).map_err(|e| vec![e])?)
        }
        _ => engine::Source::None,
    };
    let bindings = engine::resolve(&source, &requests, &handles)?;
    if matches!(source, engine::Source::Exported(_)) {
        let result = if requests.is_empty() {
            match std::fs::remove_file(lock) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        } else {
            write_if_changed(lock, &bindings.lock_json())
        };
        result.map_err(|e| vec![format!("{}: {e}", lock.display())])?;
    }
    Ok(bindings)
}

fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|old| old == contents.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

/// Writes `files` under `dir`, only those that changed, and removes the other files of the
/// `Public/Ferreal` and `Private/Ferreal` folders.
pub fn write_files(dir: &Path, files: &[GeneratedFile]) -> std::io::Result<()> {
    for f in files {
        let path = dir.join(&f.path);
        if std::fs::read(&path).is_ok_and(|old| old == f.contents.as_bytes()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &f.contents)?;
    }
    for sub in ["Public/Ferreal", "Private/Ferreal"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = format!("{sub}/{}", entry.file_name().to_string_lossy());
            if path.is_file() && !files.iter().any(|f| f.path == rel) {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}
