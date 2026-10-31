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
    /// A `cpp!` block: `tokens` are its macro's, `name` the span of its `cpp`.
    fn block(
        &mut self,
        tokens: &proc_macro2::TokenStream,
        name: proc_macro2::Span,
    ) -> syn::Result<()> {
        let block: CppBlock = syn::parse2(tokens.clone())?;
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
        self.out.cpp_blocks.push(CppAt {
            key: ferreal_model::cpp::key(tokens),
            at: at(self.file, name),
            file: self.file.to_path_buf(),
            line: name.start().line,
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
            Some("cpp") => self.block(
                &m.tokens,
                m.path.segments.last().expect("a name").ident.span(),
            ),
            Some("cpp_include") => syn::parse2::<syn::LitStr>(m.tokens.clone()).map(|s| {
                self.out.cpp_includes.push(At {
                    item: s.value(),
                    at: at(self.file, s.span()),
                });
            }),
            // Other macros' arguments are not parsed: their `cpp!` blocks are found in
            // their tokens.
            _ => {
                self.in_tokens(&m.tokens);
                Ok(())
            }
        };
        if let Err(e) = r {
            self.errors.push(located(self.file, &e));
        }
        syn::visit::visit_macro(self, m);
    }
}

impl CppFinder<'_> {
    /// The `cpp!(...)` invocations in `tokens`, at any depth. Those holding `$` (in a
    /// `macro_rules!` definition) are left out: their expansions differ.
    fn in_tokens(&mut self, tokens: &proc_macro2::TokenStream) {
        use proc_macro2::TokenTree;
        let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
        for (i, t) in trees.iter().enumerate() {
            match (t, trees.get(i + 1), trees.get(i + 2)) {
                (TokenTree::Ident(id), Some(TokenTree::Punct(bang)), Some(TokenTree::Group(g)))
                    if id == "cpp" && bang.as_char() == '!' =>
                {
                    if !g.stream().to_string().contains('$')
                        && let Err(e) = self.block(&g.stream(), id.span())
                    {
                        self.errors.push(located(self.file, &e));
                    }
                }
                (TokenTree::Group(g), _, _) => self.in_tokens(&g.stream()),
                _ => {}
            }
        }
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
    generate_with(module, api, source, model, bindings, false, false)
}

/// [`generate`]; `without_data`: the crate lacks the reflection data (`bindings` is empty);
/// `shares_types`: it depends on crates of shared engine types.
fn generate_with(
    module: &str,
    api: &str,
    source: &str,
    model: &Collected,
    bindings: &engine::Bindings,
    without_data: bool,
    shares_types: bool,
) -> Result<Vec<GeneratedFile>, Error> {
    let generator = cpp::Gen::new(module, api, source, model, bindings)
        .without_data(without_data)
        .shares_types(shares_types);
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
        println!("cargo:rerun-if-env-changed={}", engine::DEFERRED_ENV);
        let crate_dir =
            std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(PathBuf::new, PathBuf::from);
        let root = crate_dir.join(&self.root);
        let mut model = collect(&root)?;
        model.dir = crate_dir.clone();
        for f in &model.files {
            println!("cargo:rerun-if-changed={}", f.display());
        }

        let output = std::env::var_os(engine::OUTPUT_ENV)
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| self.output.as_ref().map(|o| crate_dir.join(o)));
        // The exporter first: the Unreal build it is part of exports the data the bindings
        // need.
        if let Some(output) = &output {
            write_exporter(output, &self.name)
                .map_err(|e| Error(vec![format!("{}: {e}", output.display())]))?;
        }

        // The engine bindings. Without the data they need, the rest is still written (so the
        // Unreal build that exports the data can run), and the error reported after.
        let lock = match std::env::var_os(engine::LOCK_ENV).filter(|p| !p.is_empty()) {
            Some(p) => PathBuf::from(p),
            None => crate_dir.join(&self.lock),
        };
        let (mut bindings, engine_errors) = match engine_bindings(&model, &lock) {
            Ok(b) => (b, Vec::new()),
            Err(e) => (engine::Bindings::default(), e),
        };
        // The types of the crates of shared types it depends on are theirs.
        let mut engine_errors = engine_errors;
        let shared = shared_types()?;
        if let Err(e) = bindings.share(&shared) {
            engine_errors.extend(e);
        }
        // C++ only for the engine functions the crate calls.
        bindings.keep_used(&identifiers(&model.files));
        let symbols = ferreal_model::Symbols::new(&self.name);
        if let Some(out_dir) = std::env::var_os("OUT_DIR") {
            let path = PathBuf::from(out_dir).join("ferreal_engine.rs");
            write_if_changed(&path, &bindings.rust(&symbols.register_engine()))
                .map_err(|e| Error(vec![format!("{}: {e}", path.display())]))?;
        }
        // Without the bindings, items that mention engine classes cannot be written: leave
        // the C++ as the last build wrote it, and report what is missing.
        let without_data = !engine_errors.is_empty();
        if without_data {
            if !cpp::check_without_data(&model, &bindings).is_empty() {
                return Err(Error(engine_errors));
            }
        } else {
            check(&model, &bindings)?;
        }

        if let Some(output) = &output {
            let api = self
                .api
                .clone()
                .unwrap_or_else(|| format!("{}_API", self.name.to_uppercase()));
            let source = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "the crate".into());
            let mut files = generate_with(
                &self.name,
                &api,
                &source,
                &model,
                &bindings,
                without_data,
                !shared.is_empty(),
            )?;
            // What the module depends on, from the data (without it, the list stays as the
            // last build wrote it): the modules of what the crate uses, for its Build.cs, and
            // their plugins, in its plugin's .uplugin.
            let mut plugin_modules: Vec<String> = Vec::new();
            if !without_data {
                let modules: Vec<&engine::ModuleMeta> = bindings
                    .lock
                    .modules
                    .iter()
                    .filter(|m| m.name != self.name)
                    .collect();
                if !modules.is_empty() {
                    let mut text = String::from(
                        "// Written by ferreal-build: the modules of the engine classes, structs and enums\n\
                         // the crate uses, which the module's Build.cs depends on.\n",
                    );
                    for m in &modules {
                        text.push_str(&m.name);
                        text.push('\n');
                    }
                    files.push(GeneratedFile {
                        path: MODULES_FILE.into(),
                        contents: text,
                    });
                }
                plugin_modules = modules
                    .iter()
                    .filter(|m| m.plugin.is_some())
                    .map(|m| m.name.clone())
                    .collect();
                let plugins: Vec<String> =
                    modules.iter().filter_map(|m| m.plugin.clone()).collect();
                for added in
                    add_plugin_dependencies(output, &plugins).map_err(|e| Error(vec![e]))?
                {
                    println!(
                        "cargo:warning=ferreal: {added} is now in the plugin's Plugins (the crate uses its engine classes): build again if Unreal did not enable it for this build"
                    );
                }
            }
            let io = |e: std::io::Error| Error(vec![format!("{}: {e}", output.display())]);
            if std::env::var_os(engine::DEFERRED_ENV).is_some_and(|v| v == "1") {
                write_deferred(output, &files, &plugin_modules)?;
            } else if engine_errors.is_empty() {
                write_files(output, &files).map_err(io)?;
            } else {
                write_provisional(output, &files).map_err(io)?;
            }
        }
        if engine_errors.is_empty() {
            Ok(())
        } else {
            Err(Error(engine_errors))
        }
    }
}

/// The key of the metadata a crate of shared types hands the crates depending on it: the path
/// of its manifest (they read `DEP_<links>_FERREAL_TYPES`).
const SHARED_KEY: &str = "ferreal_types";

/// The manifests of the crates of shared types this crate depends on.
fn shared_types() -> Result<Vec<engine::Shared>, Error> {
    let suffix = format!("_{}", SHARED_KEY.to_uppercase());
    let mut shared = Vec::new();
    let mut paths: Vec<(String, PathBuf)> = std::env::vars_os()
        .filter_map(|(k, v)| {
            let k = k.into_string().ok()?;
            (k.starts_with("DEP_") && k.ends_with(&suffix)).then(|| (k, PathBuf::from(v)))
        })
        .collect();
    paths.sort();
    for (_, path) in paths {
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error(vec![format!("{}: {e}", path.display())]))?;
        shared.push(
            serde_json::from_str(&text)
                .map_err(|e| Error(vec![format!("{}: {e}", path.display())]))?,
        );
    }
    Ok(shared)
}

/// The build script of a crate of engine types other crates share: its `ferreal::engine!`
/// names classes, and it holds their types (`ferreal::engine_types!()` declares its `engine`
/// module): the markers of the classes, the structs and enums their functions pass. It has no
/// functions and no C++: each Unreal module's crate depending on it binds the functions it
/// calls, with the shared types, which are then the same Rust types in every crate.
///
/// Its `Cargo.toml` has a `links` key (any name unique in the build: `links =
/// "ferreal-types-mygame"`), through which cargo hands the crates depending on it what it
/// holds.
pub struct Types {
    root: PathBuf,
    lock: PathBuf,
}

impl Default for Types {
    fn default() -> Self {
        Types::new()
    }
}

impl Types {
    pub fn new() -> Self {
        Types {
            root: PathBuf::from("src/lib.rs"),
            lock: PathBuf::from(engine::LOCK_FILE),
        }
    }

    /// The crate root, relative to the crate (default `src/lib.rs`).
    pub fn root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }

    /// The lock file, relative to the crate (default `ferreal-engine.json`).
    /// `FERREAL_ENGINE_LOCK`, if set, puts it next to that file, named after the crate.
    pub fn engine_lock(mut self, path: impl Into<PathBuf>) -> Self {
        self.lock = path.into();
        self
    }

    /// Runs as a build script; panics with every problem found.
    pub fn run(self) {
        if let Err(e) = self.try_run() {
            panic!("\n{e}");
        }
    }

    /// [`Types::run`], returning the problems instead of panicking.
    pub fn try_run(self) -> Result<(), Error> {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed={}", engine::METADATA_ENV);
        println!("cargo:rerun-if-env-changed={}", engine::LOCK_ENV);
        let name = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
        if std::env::var_os("CARGO_MANIFEST_LINKS").is_none() {
            return Err(Error(vec![format!(
                "{name}: a crate of shared engine types needs a `links` key in its Cargo.toml (`links = \"ferreal-types-{name}\"`): cargo hands the crates depending on it what it holds through it"
            )]));
        }
        let crate_dir =
            std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(PathBuf::new, PathBuf::from);
        let model = collect(&crate_dir.join(&self.root))?;
        for f in &model.files {
            println!("cargo:rerun-if-changed={}", f.display());
        }
        // Next to the overridden lock of the crates being built, apart from theirs.
        let lock = match std::env::var_os(engine::LOCK_ENV).filter(|p| !p.is_empty()) {
            Some(p) => PathBuf::from(p).with_file_name(format!("{name}.{}", engine::LOCK_FILE)),
            None => crate_dir.join(&self.lock),
        };
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
        let rust = out_dir.join("ferreal_engine.rs");
        // Without the reflection data, the build script still succeeds, with no types to hand
        // on, and the crate fails to compile instead: cargo then runs the build scripts of the
        // crates depending on it, which write their C++ (Ferreal's exporter builds them all
        // again once Unreal has exported the data).
        let (bindings, rust_text) = match engine_bindings(&model, &lock) {
            Ok(mut bindings) => {
                bindings.types_only();
                let text = bindings.rust("");
                (bindings, text)
            }
            Err(errors) if errors.iter().all(|e| e.contains(engine::NEEDS_METADATA)) => {
                let text = errors
                    .iter()
                    .map(|e| format!("compile_error!({e:?});\n"))
                    .collect();
                (engine::Bindings::default(), text)
            }
            Err(errors) => return Err(Error(errors)),
        };
        write_if_changed(&rust, &rust_text)
            .map_err(|e| Error(vec![format!("{}: {e}", rust.display())]))?;
        let manifest = out_dir.join("ferreal_types.json");
        let shared = bindings.shared(&format!("::{}::engine", name.replace('-', "_")));
        let text = serde_json::to_string_pretty(&shared).expect("serializable");
        write_if_changed(&manifest, &text)
            .map_err(|e| Error(vec![format!("{}: {e}", manifest.display())]))?;
        println!("cargo:{SHARED_KEY}={}", manifest.display());
        Ok(())
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
    // The types `cpp!` blocks pass by name that are not the crate's `#[uenum]`s.
    let own_enums: std::collections::BTreeSet<&str> =
        model.enums.iter().map(|e| e.item.name.as_str()).collect();
    let mut named: Vec<engine::Request> = Vec::new();
    for b in &model.cpp_blocks {
        let types = b.block.captures.iter().map(|c| &c.param.ty);
        for t in types.chain([&b.block.ret]) {
            if let ferreal_model::Ty::Enum(n) = t
                && !own_enums.contains(n.as_str())
                && !named.iter().any(|r| &r.cpp == n)
            {
                named.push(engine::Request {
                    cpp: n.clone(),
                    at: b.at.clone(),
                });
            }
        }
    }
    let bindings = engine::resolve(&source, &requests, &handles, &named)?;
    if matches!(source, engine::Source::Exported(_)) {
        let result = if requests.is_empty() && named.is_empty() {
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

/// Every identifier in `files` (the crate's sources), in other macros' arguments too: what
/// the crate can call. A file that does not tokenize contributes nothing (rustc reports it).
fn identifiers(files: &[PathBuf]) -> std::collections::BTreeSet<String> {
    fn walk(tokens: proc_macro2::TokenStream, out: &mut std::collections::BTreeSet<String>) {
        for t in tokens {
            match t {
                proc_macro2::TokenTree::Ident(i) => {
                    out.insert(i.to_string());
                }
                proc_macro2::TokenTree::Group(g) => walk(g.stream(), out),
                _ => {}
            }
        }
    }
    let mut out = std::collections::BTreeSet::new();
    for f in files {
        if let Ok(text) = std::fs::read_to_string(f)
            && let Ok(tokens) = text.parse::<proc_macro2::TokenStream>()
        {
            walk(tokens, &mut out);
        }
    }
    out
}

/// The exporter of Unreal Header Tool that writes the engine's reflection data, and the
/// project of Unreal Build Tool that builds it, with `{Module}` for the module's name.
const EXPORTER: &str = include_str!("../exporter/FerrealExporter.cs");
const EXPORTER_PROJECT: &str = include_str!("../exporter/Exporter.ubtplugin.csproj");

/// Writes the Ferreal exporter beside the Unreal module, in its plugin
/// (`<module>/../<Module>Exporter`): Unreal Build Tool builds it with the project, and Unreal
/// Header Tool runs it, writing `<Project>/Intermediate/Ferreal/FerrealEngine.json`. Named
/// after the module, so the copies of several plugins can be built together.
fn write_exporter(module_dir: &Path, name: &str) -> std::io::Result<()> {
    let dir = module_dir.join("..").join(format!("{name}Exporter"));
    write_if_changed(&dir.join("FerrealExporter.cs"), EXPORTER)?;
    write_if_changed(
        &dir.join("ExporterModule.cs"),
        &format!(
            "// Written by ferreal-build: the module this copy of the exporter is named after.\n\n\
             namespace Ferreal;\n\n\
             internal static class ExporterModule\n{{\n\tpublic const string Name = \"{name}\";\n}}\n"
        ),
    )?;
    write_if_changed(
        &dir.join(format!("{name}Exporter.ubtplugin.csproj")),
        &EXPORTER_PROJECT.replace("{Module}", name),
    )
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

/// Writes `files` under `dir` when the crate lacks the reflection data its engine bindings
/// need (`files` has none): Ferreal's exporter builds it again later in the Unreal build, with
/// the data, before the C++ compiles, and writes the bindings into the same files (which files
/// there are does not depend on the data). So nothing is removed: Unreal Build Tool compiles
/// the sources it listed before the exporter ran.
fn write_provisional(dir: &Path, files: &[GeneratedFile]) -> std::io::Result<()> {
    for f in files {
        let path = dir.join(&f.path);
        // A header the last build wrote with the data includes the engine's headers, which
        // this one only declares ahead: it stays.
        if f.path.starts_with("Public/")
            && std::fs::read_to_string(&path).is_ok_and(|o| same_but_includes(&o, &f.contents))
        {
            continue;
        }
        write_if_changed(&path, &f.contents)?;
    }
    Ok(())
}

/// The modules the module depends on (`Private/Ferreal/`), one a line, which its Build.cs
/// reads.
const MODULES_FILE: &str = "Private/Ferreal/FerrealModules.txt";

/// Whether two versions of a header differ only in what they include and declare ahead
/// (`class AActor;`), and blank lines: what Unreal Header Tool does not read.
fn same_but_includes(a: &str, b: &str) -> bool {
    fn kept(text: &str) -> impl Iterator<Item = &str> {
        text.lines().map(str::trim).filter(|l| {
            !l.is_empty()
                && !l.starts_with("#include ")
                && !(l.starts_with("class ")
                    && l.ends_with(';')
                    && l.split_whitespace().count() == 2)
        })
    }
    kept(a).eq(kept(b))
}

/// Adds `plugins` to the `Plugins` of the `.uplugin` of the module at `module_dir` (its
/// plugin's, `<Plugin>/Source/<Module>`) that it does not list, but its own; none without one
/// (a game's module). Leaves the rest of the file as it is. Returns those added.
fn add_plugin_dependencies(module_dir: &Path, plugins: &[String]) -> Result<Vec<String>, String> {
    let root = module_dir.join("..").join("..");
    let Some(descriptor) = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "uplugin"))
    else {
        return Ok(Vec::new());
    };
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", descriptor.display());
    let own = descriptor
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut text = std::fs::read_to_string(&descriptor).map_err(|e| at(&e))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| at(&e))?;
    let listed: Vec<&str> = json["Plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["Name"].as_str())
        .collect();
    let mut missing: Vec<String> = Vec::new();
    for p in plugins {
        if *p != own && !listed.contains(&p.as_str()) && !missing.contains(p) {
            missing.push(p.clone());
        }
    }
    if missing.is_empty() {
        return Ok(missing);
    }
    let entries = missing
        .iter()
        .map(|n| format!("\t\t{{\n\t\t\t\"Name\": \"{n}\",\n\t\t\t\"Enabled\": true\n\t\t}}"))
        .collect::<Vec<_>>()
        .join(",\n");
    let key = text.find("\"Plugins\"");
    match key.and_then(|k| text[k..].find('[').map(|o| k + o)) {
        Some(open) => {
            let empty = text[open + 1..].trim_start().starts_with(']');
            let insert = if empty {
                format!("\n{entries}\n\t")
            } else {
                format!("\n{entries},")
            };
            text.insert_str(open + 1, &insert);
        }
        None => {
            let close = text.rfind('}').ok_or_else(|| at(&"not a JSON object"))?;
            let last = text[..close].trim_end().len();
            text.insert_str(last, &format!(",\n\t\"Plugins\": [\n{entries}\n\t]"));
        }
    }
    serde_json::from_str::<serde_json::Value>(&text).map_err(|e| at(&e))?;
    std::fs::write(&descriptor, text).map_err(|e| at(&e))?;
    Ok(missing)
}

/// Writes `files` under `dir` from Ferreal's exporter, in the middle of an Unreal build:
/// Unreal Header Tool has read the headers, and Unreal Build Tool has listed the sources it
/// compiles. So nothing is removed, and a header that changes or a source that is new fails
/// the build (written all the same, for the next), as does a dependency on a module of a
/// plugin (of `plugin_modules`) new to the modules the Build.cs read: the engine's own come
/// with its Engine module.
fn write_deferred(
    dir: &Path,
    files: &[GeneratedFile],
    plugin_modules: &[String],
) -> Result<(), Error> {
    let mut late = Vec::new();
    for f in files {
        let path = dir.join(&f.path);
        let old = std::fs::read_to_string(&path).ok();
        if old.as_deref() == Some(f.contents.as_str()) {
            continue;
        }
        // A header may change in what it includes (Unreal Header Tool does not read that),
        // not in what it declares.
        let header_late = f.path.starts_with("Public/")
            && !old
                .as_deref()
                .is_some_and(|o| same_but_includes(o, &f.contents));
        let modules_late = f.path == MODULES_FILE
            && plugin_modules.iter().any(|m| {
                !old.as_deref()
                    .is_some_and(|o| o.lines().any(|l| l.trim() == m))
            });
        if (old.is_none() && f.path != MODULES_FILE) || header_late || modules_late {
            late.push(f.path.clone());
        }
        write_if_changed(&path, &f.contents)
            .map_err(|e| Error(vec![format!("{}: {e}", path.display())]))?;
    }
    if late.is_empty() {
        Ok(())
    } else {
        Err(Error(vec![format!(
            "{}: {} changed with the reflection data Unreal just exported, after this build read the module: build again",
            dir.display(),
            late.join(", ")
        )]))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_differing_in_includes() {
        let ahead = "#pragma once\n#include \"CoreMinimal.h\"\nclass AActor;\n\nUCLASS()\nclass UThing : public UObject\n{\n};\n";
        let included = "#pragma once\n#include \"CoreMinimal.h\"\n#include \"GameFramework/Actor.h\"\nUCLASS()\nclass UThing : public UObject\n{\n};\n";
        assert!(same_but_includes(ahead, included));
        assert!(!same_but_includes(
            ahead,
            &included.replace("UThing", "UOther")
        ));
    }

    #[test]
    fn plugin_dependencies() {
        let dir = std::env::temp_dir().join(format!("ferreal-uplugin-{}", std::process::id()));
        let module = dir.join("Source").join("Mine");
        std::fs::create_dir_all(&module).unwrap();
        let descriptor = dir.join("Mine.uplugin");
        let plugins = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        let listed = |path: &Path| -> Vec<String> {
            let json: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            json["Plugins"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["Name"].as_str().unwrap().to_string())
                .collect()
        };

        // No Plugins yet: the key is added; its own plugin is not.
        std::fs::write(
            &descriptor,
            "{\n\t\"FileVersion\": 3,\n\t\"Modules\": []\n}\n",
        )
        .unwrap();
        let added = add_plugin_dependencies(&module, &plugins(&["EnhancedInput", "Mine"])).unwrap();
        assert_eq!(added, ["EnhancedInput"]);
        assert_eq!(listed(&descriptor), ["EnhancedInput"]);

        // Listed ones stay; others go first in the list.
        let added =
            add_plugin_dependencies(&module, &plugins(&["EnhancedInput", "Niagara"])).unwrap();
        assert_eq!(added, ["Niagara"]);
        assert_eq!(listed(&descriptor), ["Niagara", "EnhancedInput"]);
        assert!(
            add_plugin_dependencies(&module, &plugins(&["Niagara"]))
                .unwrap()
                .is_empty()
        );

        // An empty list.
        std::fs::write(&descriptor, "{\n\t\"Plugins\": []\n}\n").unwrap();
        add_plugin_dependencies(&module, &plugins(&["Niagara"])).unwrap();
        assert_eq!(listed(&descriptor), ["Niagara"]);

        // A game's module, without a .uplugin.
        std::fs::remove_file(&descriptor).unwrap();
        assert!(
            add_plugin_dependencies(&module, &plugins(&["Niagara"]))
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
