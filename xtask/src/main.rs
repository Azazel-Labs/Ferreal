//! The repository's development tasks: `cargo xtask <task>`.
//!
//! | Task | |
//! |---|---|
//! | `mock-ue [--cxx <compiler>]` | Builds the example, compiles its C++ against the mock engine headers, links the Rust and runs the test. |
//! | `exporter` | Runs the exporter over the made-up session of Unreal Header Tool, writing the mock engine's reflection data (needs the .NET SDK). |
//! | `test` | `cargo test`, then `exporter` and `mock-ue`. |
//! | `lint` | `cargo fmt --check` and `cargo clippy`, warnings as errors. |

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

type Result<T = ()> = std::result::Result<T, String>;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
  mock-ue [--cxx <compiler>]  build the example and run its C++ against the mock engine
                              (compiler: --cxx, else $CXX, else clang++ on Windows, c++ elsewhere)
  exporter                    run the exporter over the made-up UHT session (needs the .NET SDK)
  test                        cargo test, then exporter and mock-ue
  lint                        cargo fmt --check and cargo clippy, warnings as errors";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["mock-ue"] => mock_ue(None),
        ["mock-ue", "--cxx", cxx] => mock_ue(Some(cxx)),
        ["exporter"] => exporter(),
        ["test"] => test(),
        ["lint"] => lint(),
        ["help" | "--help" | "-h"] | [] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("unknown task `{}`\n\n{USAGE}", args.join(" "))),
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        process::exit(1);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cargo() -> Command {
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.current_dir(root());
    command
}

fn run(command: &mut Command) -> Result {
    let status = command
        .status()
        .map_err(|e| format!("could not run {:?}: {e}", command.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{:?} failed ({status})", command.get_program()))
    }
}

fn test() -> Result {
    run(cargo().args(["test", "--workspace"]))?;
    exporter()?;
    mock_ue(None)
}

fn lint() -> Result {
    run(cargo().args(["fmt", "--all", "--", "--check"]))?;
    run(cargo().args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))
}

/// Builds and runs the exporter over the made-up session (Fixture.cs), writing the reflection
/// data the mock engine test reads.
fn exporter() -> Result {
    let tests = root().join("tests");
    run(Command::new("dotnet")
        .arg("run")
        .arg("--project")
        .arg(tests.join("exporter/exporter.csproj"))
        .arg("--")
        .arg(tests.join("mock-ue/FerrealEngine.json")))
}

/// Builds the example crate (which writes the C++ glue), compiles the example module's C++
/// against the mock engine headers, links the Rust static library and runs the test.
fn mock_ue(cxx: Option<&str>) -> Result {
    let root = root();
    let here = root.join("tests/mock-ue");
    let module = root.join("examples/hello/Source/HelloRust");
    let target = env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let out = target.join("mock-ue");

    // The engine's reflection data comes from the exporter's output for the made-up session of
    // tests/exporter (as an Unreal build passes FerrealEngine.json); the build refreshes the
    // example's lock file from it. Building through `cargo rustc` also reports the system
    // libraries the static library needs.
    let build = cargo()
        .args([
            "rustc",
            "--quiet",
            "-p",
            "ferreal-hello",
            "--lib",
            "--crate-type",
            "staticlib",
        ])
        .args(["--", "--print", "native-static-libs"])
        .env("FERREAL_ENGINE_METADATA", here.join("FerrealEngine.json"))
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    let stderr = String::from_utf8_lossy(&build.stderr);
    if !build.status.success() {
        return Err(format!("building ferreal-hello failed:\n{stderr}"));
    }
    let native_libs = stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix("note: native-static-libs:"))
        .ok_or("cargo did not report the static library's native libraries")?;
    let msvc = cfg!(target_env = "msvc");
    let lib = target.join("debug").join(if msvc {
        "ferreal_hello.lib"
    } else {
        "libferreal_hello.a"
    });

    // Unreal Header Tool's generated headers, imitated: each defines GENERATED_BODY with the
    // class' Super.
    let generated = out.join("gen");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("could not remove {}: {e}", out.display()))?;
    }
    fs::create_dir_all(&generated)
        .map_err(|e| format!("could not create {}: {e}", generated.display()))?;
    for header in files(&module.join("Public"), "h")? {
        let source = read(&header)?;
        let body = match parent_class(&source) {
            Some(parent) => format!("public: using Super = {parent}; private:"),
            None => String::new(),
        };
        let name = header.file_stem().unwrap().to_string_lossy();
        let path = generated.join(format!("{name}.generated.h"));
        fs::write(
            &path,
            format!("#undef GENERATED_BODY\n#define GENERATED_BODY() {body}\n"),
        )
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    }

    let cxx = cxx
        .map(Into::into)
        .or_else(|| env::var_os("CXX"))
        .unwrap_or_else(|| if cfg!(windows) { "clang++" } else { "c++" }.into());
    let exe = out.join(format!("test_hello{}", env::consts::EXE_SUFFIX));
    let mut compile = Command::new(&cxx);
    compile.args([
        "-std=c++20",
        "-Wall",
        "-Wextra",
        "-Wshadow",
        "-Wno-unused-parameter",
    ]);
    compile.args(["-Wno-unused-variable", "-Werror", "-DHELLORUST_API="]);
    if msvc {
        // Rust links the DLL C runtime; the clang driver would add the static one too.
        compile.args(["-fms-runtime-lib=dll", "-Xlinker", "/NODEFAULTLIB:libcmt"]);
    }
    for include in [here.join("include"), generated, module.join("Public")] {
        compile.arg("-I").arg(include);
    }
    compile.args(files(&module.join("Private"), "cpp")?);
    compile.arg(here.join("test_hello.cpp")).arg(lib);
    for native in native_libs.split_whitespace() {
        match native.strip_suffix(".lib") {
            Some(name) => compile.arg(format!("-l{name}")),
            // `/defaultlib:msvcrt`: the runtime, chosen above.
            None if native.starts_with('/') => continue,
            None => compile.arg(native),
        };
    }
    compile.arg("-o").arg(&exe);
    run(&mut compile)?;
    run(&mut Command::new(&exe))
}

/// The parent of the first `class API Name : public Parent` in a header.
fn parent_class(source: &str) -> Option<&str> {
    let words: Vec<&str> = source.split_whitespace().collect();
    words.windows(6).find_map(|w| match w {
        ["class", _, _, ":", "public", parent] => {
            let end = parent
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(parent.len());
            Some(&parent[..end])
        }
        _ => None,
    })
}

/// The files with an extension under a directory, recursively, sorted.
fn files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let entries =
        fs::read_dir(dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("could not read {}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            found.extend(files(&path, extension)?);
        } else if path.extension() == Some(OsStr::new(extension)) {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|e| format!("could not read {}: {e}", path.display()))
}
