//! The repository's development tasks: `cargo xtask <task>`.
//!
//! | Task | |
//! |---|---|
//! | `mock-ue [--cxx <compiler>]` | Builds the example, compiles its C++ against the mock engine headers, links the Rust and runs the test. |
//! | `exporter` | Runs the exporter over the made-up session of Unreal Header Tool, writing the mock engine's reflection data (needs the .NET SDK). |
//! | `test` | `cargo test`, then `exporter` and `mock-ue`. |
//! | `lint` | `cargo fmt --check` and `cargo clippy`, warnings as errors. |
//! | `ue [--engine <dir>]` | Builds the host project (tests/ue-host), with the plugin and the example, in a real Unreal Engine. |

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

type Result<T = ()> = std::result::Result<T, String>;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
  mock-ue [--cxx <compiler>]  build the example and run its C++ against the mock engine
                              (compiler: --cxx, else $CXX, else clang++ on Windows, c++ elsewhere)
  exporter                    run the exporter over the made-up UHT session (needs the .NET SDK)
  test                        cargo test, then exporter and mock-ue
  lint                        cargo fmt --check and cargo clippy, warnings as errors
  ue [--engine <dir>]         build tests/ue-host's editor target, with the Ferreal plugin and the
                              example, in a real engine (engine: --engine, else $UE_ROOT, else
                              the launcher's default UE_5.8)";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["mock-ue"] => mock_ue(None),
        ["mock-ue", "--cxx", cxx] => mock_ue(Some(cxx)),
        ["exporter"] => exporter(),
        ["test"] => test(),
        ["lint"] => lint(),
        ["ue"] => ue(None),
        ["ue", "--engine", engine] => ue(Some(engine)),
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

/// Builds the host project's editor target with Unreal Build Tool, its Plugins folder linking
/// to the Ferreal plugin and the example. A build that exports reflection data the example
/// had not had yet asks for another (see HelloRust.Build.cs), which this runs.
fn ue(engine: Option<&str>) -> Result {
    let root = root();
    let engine = engine
        .map(PathBuf::from)
        .or_else(|| env::var_os("UE_ROOT").map(PathBuf::from));
    let engine = match engine {
        Some(engine) => engine,
        None if cfg!(windows) => PathBuf::from(r"C:\Program Files\Epic Games\UE_5.8"),
        None if cfg!(target_os = "macos") => PathBuf::from("/Users/Shared/Epic Games/UE_5.8"),
        None => return Err("no engine: pass --engine <dir> or set UE_ROOT".into()),
    };
    let batch_files = engine.join("Engine/Build/BatchFiles");
    let (build, platform) = if cfg!(windows) {
        (batch_files.join("Build.bat"), "Win64")
    } else if cfg!(target_os = "macos") {
        (batch_files.join("Mac/Build.sh"), "Mac")
    } else {
        (batch_files.join("Linux/Build.sh"), "Linux")
    };
    if !build.exists() {
        return Err(format!(
            "no engine at {} ({} is missing)",
            engine.display(),
            build.display()
        ));
    }

    let host = root.join("tests/ue-host");
    let plugins = host.join("Plugins");
    fs::create_dir_all(&plugins)
        .map_err(|e| format!("could not create {}: {e}", plugins.display()))?;
    link_dir(&root.join("unreal/Ferreal"), &plugins.join("Ferreal"))?;
    link_dir(&root.join("examples/hello"), &plugins.join("HelloRust"))?;

    for _ in 0..2 {
        let mut child = Command::new(&build)
            .args(["FerrealHostEditor", platform, "Development", "-WaitMutex"])
            .arg(format!(
                "-Project={}",
                host.join("FerrealHost.uproject").display()
            ))
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not run {}: {e}", build.display()))?;
        let mut again = false;
        for line in BufReader::new(child.stdout.take().unwrap()).split(b'\n') {
            let line = line.map_err(|e| format!("could not read the build's output: {e}"))?;
            let line = String::from_utf8_lossy(&line);
            again |= line.contains("Build again to generate the bindings");
            println!("{}", line.trim_end());
        }
        let status = child
            .wait()
            .map_err(|e| format!("could not wait for the build: {e}"))?;
        if !status.success() {
            return Err(format!("the Unreal build failed ({status})"));
        }
        if !again {
            return Ok(());
        }
        println!("xtask: building again, with the exported reflection data");
    }
    Err("the second build still had no reflection data for the example's engine classes".into())
}

/// Makes `link` a link to the directory `target` (a junction on Windows, which needs no
/// privileges), unless it exists.
fn link_dir(target: &Path, link: &Path) -> Result {
    if link.exists() {
        return Ok(());
    }
    // mklink takes backslashes only.
    #[cfg(windows)]
    run(Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link.components().collect::<PathBuf>())
        .arg(target.components().collect::<PathBuf>())
        .stdout(Stdio::null()))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).map_err(|e| {
        format!(
            "could not link {} to {}: {e}",
            link.display(),
            target.display()
        )
    })?;
    Ok(())
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
    // The glue and lock file of the mock engine's data, kept out of the example's (which are
    // the real engine's); not cleaned, as cargo writes them only when they change.
    let glue = target.join("mock-ue-glue");

    // The engine's reflection data comes from the exporter's output for the made-up session of
    // tests/exporter (as an Unreal build passes FerrealEngine.json). Building through
    // `cargo rustc` also reports the system libraries the static library needs.
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
        .env("FERREAL_ENGINE_LOCK", glue.join("ferreal-engine.json"))
        .env("FERREAL_OUTPUT", &glue)
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
    // The module's hand-written files, and the glue (its generated folders are the real
    // engine's).
    let hand_written = |dir: &str, extension| -> Result<Vec<PathBuf>> {
        let dir = module.join(dir);
        let generated = dir.join("Ferreal");
        Ok(files(&dir, extension)?
            .into_iter()
            .filter(|f| !f.starts_with(&generated))
            .collect())
    };
    let headers = hand_written("Public", "h")?
        .into_iter()
        .chain(files(&glue.join("Public"), "h")?);
    for header in headers {
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
    for include in [
        here.join("include"),
        generated,
        glue.join("Public"),
        module.join("Public"),
    ] {
        compile.arg("-I").arg(include);
    }
    compile.args(hand_written("Private", "cpp")?);
    compile.args(files(&glue.join("Private"), "cpp")?);
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
