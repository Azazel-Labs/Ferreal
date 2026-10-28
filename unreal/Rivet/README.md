# The Rivet plugin

Copy this folder into a project's `Plugins` folder to use `rivet::engine!`: it holds the
exporter that writes Unreal's reflection data for Rivet.

* `Source/RivetExporter`: a plugin of Unreal Build Tool (`RivetExporter.ubtplugin.csproj`),
  which UBT builds with the project. Its exporter runs while Unreal Header Tool runs, in
  every build, and writes the classes, functions, parameters and enums of everything UHT
  parsed (the engine, the project, its plugins) to `RivetEngine.json`.
* `Source/RivetMetadata`: an empty editor module, the home of that output:
  `Intermediate/Build/<Platform>/<Target>/Inc/RivetMetadata/UHT/RivetEngine.json`.

The Build.cs template of Rivet's crates finds the newest `RivetEngine.json` under the
project's plugins and passes it to cargo (`RIVET_ENGINE_METADATA`), and `rivet-build`
generates the bindings of the classes a crate names. See Rivet's README, "Calling engine
classes".

The exporter writes what a call through reflection needs and nothing about memory layout:
the generated code lays parameters out with the C++ compiler and checks that layout against
the engine's reflection before the first call.
