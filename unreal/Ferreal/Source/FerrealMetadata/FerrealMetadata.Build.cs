// An empty module (but for the enum Unreal Header Tool needs to run exporters for it): the
// home of the reflection data the Ferreal exporter (../FerrealExporter, a plugin of Unreal
// Build Tool) writes while Unreal Header Tool runs. The exporter's output lands in this
// module's generated folder:
// Intermediate/Build/<Platform>/<Target>/Inc/FerrealMetadata/UHT/FerrealEngine.json.

using UnrealBuildTool;

public class FerrealMetadata : ModuleRules
{
	public FerrealMetadata(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PrivateDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject" });
	}
}
