//! The repository's development tasks: `cargo xtask <task>`.
//!
//! | Task | |
//! |---|---|
//! | `mock-ue [--cxx <compiler>]` | Builds the test plugin's crate (tests/hello), compiles its C++ against the mock engine headers, links the Rust and runs the test. |
//! | `exporter` | Runs the exporter over the made-up session of Unreal Header Tool, writing the mock engine's reflection data (needs the .NET SDK). |
//! | `test` | `exporter`, then `cargo test` and `mock-ue`. |
//! | `lint` | `cargo fmt --check` and `cargo clippy`, warnings as errors. |
//! | `ue [--engine <dir>] [--test]` | Builds the host project (tests/ue-host), with the test plugin, in a real Unreal Engine; `--test` then runs its test in a headless editor. |
//! | `game [--engine <dir>] [--check \| --play]` | Builds the example game (examples/UnrealGame) and its Rust plugin, FerrealSample; `--check` then plays it headless, the player taken from orb to orb, and checks two rounds are won; `--play` opens it in a window. |
//! | `editor [--engine <dir>]` | Builds the example game, then opens it in the Unreal Editor. |

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

type Result<T = ()> = std::result::Result<T, String>;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
  mock-ue [--cxx <compiler>]  build tests/hello and run its C++ against the mock engine
                              (compiler: --cxx, else $CXX, else clang++ on Windows, c++ elsewhere)
  exporter                    run the exporter over the made-up UHT session (needs the .NET SDK)
  test                        exporter, then cargo test and mock-ue
  lint                        cargo fmt --check and cargo clippy, warnings as errors
  ue [--engine <dir>] [--test]
                              build tests/ue-host's editor target, with the test plugin
                              (tests/hello), in a real engine (engine: --engine, else $UE_ROOT,
                              else the launcher's default UE_5.8); --test then runs the host's
                              test (Ferreal.*) in a headless editor
  game [--engine <dir>] [--check | --play]
                              build examples/UnrealGame's editor target, with its Rust plugin;
                              --check then plays it headless with the autopilot and checks that
                              rounds are won, --play opens the game in a window
  editor [--engine <dir>]     build examples/UnrealGame, then open it in the Unreal Editor";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["mock-ue"] => mock_ue(None),
        ["mock-ue", "--cxx", cxx] => mock_ue(Some(cxx)),
        ["exporter"] => exporter(),
        ["test"] => test(),
        ["lint"] => lint(),
        ["ue", rest @ ..] => match rest {
            [] => ue(None, false),
            ["--test"] => ue(None, true),
            ["--engine", engine] => ue(Some(engine), false),
            ["--engine", engine, "--test"] | ["--test", "--engine", engine] => {
                ue(Some(engine), true)
            }
            _ => Err(format!(
                "unknown arguments to ue: {}\n\n{USAGE}",
                rest.join(" ")
            )),
        },
        ["game", rest @ ..] => {
            let (engine, rest) = match rest {
                ["--engine", engine, rest @ ..] => (Some(*engine), rest),
                rest => (None, rest),
            };
            match rest {
                [] => game(engine, None),
                ["--check"] => game(engine, Some(Play::Check)),
                ["--play"] => game(engine, Some(Play::Window)),
                _ => Err(format!(
                    "unknown arguments to game: {}\n\n{USAGE}",
                    rest.join(" ")
                )),
            }
        }
        ["editor"] => game(None, Some(Play::Editor)),
        ["editor", "--engine", engine] => game(Some(engine), Some(Play::Editor)),
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

/// Builds the test plugin's crate (tests/hello) against the mock engine's reflection data
/// (the exporter's output for the made-up session of tests/exporter, as an Unreal build
/// passes FerrealEngine.json): the repository holds no export of a real engine. Its glue and
/// lock file go to `target/mock-ue-glue`, apart from its own (an Unreal build's); cargo
/// writes them only when they change.
fn mock_engine(command: &mut Command) -> &mut Command {
    let root = root();
    let glue = target_dir().join("mock-ue-glue");
    command
        .env(
            "FERREAL_ENGINE_METADATA",
            root.join("tests/mock-ue/FerrealEngine.json"),
        )
        .env("FERREAL_ENGINE_LOCK", glue.join("ferreal-engine.json"))
        .env("FERREAL_OUTPUT", glue)
}

fn target_dir() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR").map_or_else(|| root().join("target"), PathBuf::from)
}

fn test() -> Result {
    // The mock engine's data first: the test plugin's build reads it.
    exporter()?;
    run(mock_engine(cargo().args(["test", "--workspace"])))?;
    mock_ue(None)
}

fn lint() -> Result {
    run(cargo().args(["fmt", "--all", "--", "--check"]))?;
    run(mock_engine(cargo().args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])))
}

/// Builds the host project's editor target with Unreal Build Tool, its Plugins folder linking
/// to the test plugin (tests/hello). A build that exports reflection data the plugin had not
/// had yet asks for another (see HelloRust.Build.cs), which this runs.
fn ue(engine: Option<&str>, test: bool) -> Result {
    let root = root();
    let (engine, platform) = engine_dir(engine)?;
    let host = root.join("tests/ue-host");
    let plugins = host.join("Plugins");
    fs::create_dir_all(&plugins)
        .map_err(|e| format!("could not create {}: {e}", plugins.display()))?;
    link_dir(&root.join("tests/hello"), &plugins.join("HelloRust"))?;
    build_project(
        &engine,
        platform,
        &host.join("FerrealHost.uproject"),
        "FerrealHostEditor",
    )?;
    if test {
        ue_test(&engine, platform, &host)
    } else {
        Ok(())
    }
}

enum Play {
    Check,
    Window,
    Editor,
}

/// Builds the example game's editor target, with its Rust plugin (Plugins/FerrealSample); then
/// plays it, or opens it in the editor, if asked.
fn game(engine: Option<&str>, play: Option<Play>) -> Result {
    let root = root();
    let (engine, platform) = engine_dir(engine)?;
    let game = root.join("examples/UnrealGame");
    let project = game.join("UnrealGame.uproject");
    build_project(&engine, platform, &project, "UnrealGameEditor")?;
    match play {
        None => Ok(()),
        Some(Play::Window) => {
            let editor = engine.join(format!(
                "Engine/Binaries/{platform}/UnrealEditor{}",
                env::consts::EXE_SUFFIX
            ));
            run(Command::new(editor).arg(&project).args([
                "-game",
                "-windowed",
                "-ResX=1280",
                "-ResY=720",
            ]))
        }
        Some(Play::Check) => game_check(&engine, platform, &project),
        // The editor runs on after this task ends.
        Some(Play::Editor) => {
            let editor = engine.join(format!(
                "Engine/Binaries/{platform}/UnrealEditor{}",
                env::consts::EXE_SUFFIX
            ));
            Command::new(&editor)
                .arg(&project)
                .spawn()
                .map_err(|e| format!("could not run {}: {e}", editor.display()))?;
            println!(
                "xtask: opened {} in {}",
                project.display(),
                editor.display()
            );
            Ok(())
        }
    }
}

/// Plays the game headless with `-OrbAutopilot` (the Rust takes the player from orb to orb)
/// until two rounds are won, failing on errors Ferreal logs or after five minutes (the first
/// run of a fresh checkout compiles shaders).
fn game_check(engine: &Path, platform: &str, project: &Path) -> Result {
    let editor = engine.join(format!(
        "Engine/Binaries/{platform}/UnrealEditor-Cmd{}",
        env::consts::EXE_SUFFIX
    ));
    println!("xtask: playing the game in {}", editor.display());
    let mut child = Command::new(&editor)
        .arg(project)
        .args([
            "-game",
            "-nullrhi",
            "-unattended",
            "-nosplash",
            "-nosound",
            "-stdout",
            "-FullStdOutLogOutput",
            "-OrbAutopilot",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {}: {e}", editor.display()))?;
    let stdout = child.stdout.take().unwrap();
    // The game runs until stopped: its lines come from a thread, read against a deadline.
    let (lines, received) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).split(b'\n').map_while(|l| l.ok()) {
            let line = String::from_utf8_lossy(&line).trim_end().to_string();
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut result = Err("the game did not win two rounds in five minutes".to_string());
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(line) = received.recv_timeout(left) else {
            break;
        };
        if line.contains("LogFerreal") || line.contains("[OrbGame") {
            println!("{line}");
        }
        if line.contains("LogFerreal") && line.contains("Error") {
            result = Err(format!("Ferreal logged an error: {line}"));
            break;
        }
        if line.contains("Round 2 done") {
            result = Ok(());
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    if result.is_ok() {
        println!("xtask: the game ran, and the Rust won two rounds");
    }
    result
}

/// The engine to build with (`--engine`, else `UE_ROOT`, else the launcher's default), and
/// the platform's name.
fn engine_dir(engine: Option<&str>) -> Result<(PathBuf, &'static str)> {
    let engine = engine
        .map(PathBuf::from)
        .or_else(|| env::var_os("UE_ROOT").map(PathBuf::from));
    let engine = match engine {
        Some(engine) => engine,
        None if cfg!(windows) => PathBuf::from(r"C:\Program Files\Epic Games\UE_5.8"),
        None if cfg!(target_os = "macos") => PathBuf::from("/Users/Shared/Epic Games/UE_5.8"),
        None => return Err("no engine: pass --engine <dir> or set UE_ROOT".into()),
    };
    let platform = if cfg!(windows) {
        "Win64"
    } else if cfg!(target_os = "macos") {
        "Mac"
    } else {
        "Linux"
    };
    let build = build_tool(&engine);
    if !build.exists() {
        return Err(format!(
            "no engine at {} ({} is missing)",
            engine.display(),
            build.display()
        ));
    }
    Ok((engine, platform))
}

/// The engine's script that runs Unreal Build Tool.
fn build_tool(engine: &Path) -> PathBuf {
    let batch_files = engine.join("Engine/Build/BatchFiles");
    if cfg!(windows) {
        batch_files.join("Build.bat")
    } else if cfg!(target_os = "macos") {
        batch_files.join("Mac/Build.sh")
    } else {
        batch_files.join("Linux/Build.sh")
    }
}

/// Builds `target` of `project` with Unreal Build Tool, once: a build succeeds without the
/// reflection data the Rust needs exported beforehand (Ferreal's exporter builds the Rust once
/// it has exported it), as on a build farm.
fn build_project(engine: &Path, platform: &str, project: &Path, target: &str) -> Result {
    let build = build_tool(engine);
    run(Command::new(&build)
        .args([target, platform, "Development", "-WaitMutex"])
        .arg(format!("-Project={}", project.display())))
}

/// Runs the host project's automation tests (`Ferreal.*`, FerrealHostTest.cpp) in a headless
/// editor, which fail on the errors Ferreal logs.
fn ue_test(engine: &Path, platform: &str, host: &Path) -> Result {
    let editor = engine.join(format!(
        "Engine/Binaries/{platform}/UnrealEditor-Cmd{}",
        env::consts::EXE_SUFFIX
    ));
    let mut command = Command::new(&editor);
    command.arg(host.join("FerrealHost.uproject")).args([
        "-unattended",
        "-nullrhi",
        "-nosplash",
        "-nosound",
        "-nop4",
        "-stdout",
        "-FullStdOutLogOutput",
    ]);
    // Values with spaces, quoted as Unreal parses them.
    let quoted = [
        "-ExecCmds=\"Automation RunTests Ferreal\"",
        "-TestExit=\"Automation Test Queue Empty\"",
    ];
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        for a in quoted {
            command.raw_arg(a);
        }
    }
    #[cfg(not(windows))]
    command.args(quoted.map(|a| a.replace('"', "")));
    println!("xtask: running the tests in {}", editor.display());
    let mut child = command
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {}: {e}", editor.display()))?;
    let (mut passed, mut failed) = (0, 0);
    for line in BufReader::new(child.stdout.take().unwrap()).split(b'\n') {
        let line = line.map_err(|e| format!("could not read the editor's output: {e}"))?;
        let line = String::from_utf8_lossy(&line);
        let line = line.trim_end();
        // The editor says much; the tests' lines and errors are what matter here.
        if line.contains("Result={Success}") {
            passed += 1;
        } else if line.contains("Result={") {
            failed += 1;
        }
        if line.contains("LogAutomation") || line.contains("Ferreal") || line.contains("Error:") {
            println!("{line}");
        }
    }
    let status = child
        .wait()
        .map_err(|e| format!("could not wait for the editor: {e}"))?;
    match (passed, failed) {
        (0, 0) => Err(format!("no test ran (the editor exited with {status})")),
        (p, 0) => {
            println!("xtask: {p} test(s) passed in the engine");
            Ok(())
        }
        (_, f) => Err(format!("{f} test(s) failed in the engine")),
    }
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

/// Builds the test plugin's crate (which writes the C++ glue), compiles its module's C++
/// against the mock engine headers, links the Rust static library and runs the test.
fn mock_ue(cxx: Option<&str>) -> Result {
    let root = root();
    let here = root.join("tests/mock-ue");
    let module = root.join("tests/hello/Source/HelloRust");
    let target = target_dir();
    let out = target.join("mock-ue");
    let glue = target.join("mock-ue-glue");

    // Building through `cargo rustc` also reports the system libraries the static library
    // needs.
    let build = mock_engine(&mut cargo())
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
