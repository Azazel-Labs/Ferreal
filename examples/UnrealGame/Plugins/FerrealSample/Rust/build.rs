//! Writes the C++ half of the glue into the plugin's module (../Source/FerrealSample).

fn main() {
    ferreal_build::Module::new("FerrealSample")
        .output("../Source/FerrealSample")
        .run();
}
