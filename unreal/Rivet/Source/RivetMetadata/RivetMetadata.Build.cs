// An empty module: the home of the reflection data the Rivet exporter (../RivetExporter, a
// plugin of Unreal Build Tool) writes while Unreal Header Tool runs. The exporter's output
// lands in this module's generated folder:
// Intermediate/Build/<Platform>/<Target>/Inc/RivetMetadata/UHT/RivetEngine.json.

using UnrealBuildTool;

public class RivetMetadata : ModuleRules
{
	public RivetMetadata(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PrivateDependencyModuleNames.AddRange(new string[] { "Core" });
	}
}
