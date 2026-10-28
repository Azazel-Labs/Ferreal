//! Writes the C++ half of the glue into the HelloRust module (../Source/HelloRust).

fn main() {
    rivet_build::Module::new("HelloRust")
        .output("../Source/HelloRust")
        .run();
}
