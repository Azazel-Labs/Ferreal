// The host project's module: a test of the example in the real engine (FerrealHostTest.cpp).
// The project exists to build Ferreal's plugins, which cargo xtask ue links into its Plugins
// folder.

using UnrealBuildTool;

public class FerrealHost : ModuleRules
{
	public FerrealHost(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine", "HelloRust" });
	}
}
