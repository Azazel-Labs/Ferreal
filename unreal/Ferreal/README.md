# The Ferreal plugin

Copy this folder into a project's `Plugins` folder to use `ferreal::engine!`: it holds the
exporter that writes Unreal's reflection data for Ferreal.

* `Source/FerrealExporter`: a plugin of Unreal Build Tool (`FerrealExporter.ubtplugin.csproj`),
  which UBT builds with the project. Its exporter runs while Unreal Header Tool runs, in
  every build, and writes the classes, functions, parameters and enums of everything UHT
  parsed (the engine, the project, its plugins) to `FerrealEngine.json`.
* `Source/FerrealMetadata`: an empty editor module, the home of that output:
  `Intermediate/Build/<Platform>/<Target>/Inc/FerrealMetadata/UHT/FerrealEngine.json`.

The Build.cs template of Ferreal's crates finds the newest `FerrealEngine.json` under the
project's plugins and passes it to cargo (`FERREAL_ENGINE_METADATA`), and `ferreal-build`
generates the bindings of the classes a crate names. See Ferreal's README, "Calling engine
classes".

The exporter writes what a call through reflection needs and nothing about memory layout:
the generated code lays parameters out with the C++ compiler and checks that layout against
the engine's reflection before the first call.
