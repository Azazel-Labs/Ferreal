// The game's module, in C++: it uses the FerrealSample plugin (written in Rust, with Ferreal) as it
// would any C++ plugin, through its module and headers.

using UnrealBuildTool;

public class UnrealGame : ModuleRules
{
	public UnrealGame(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
		PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine" });
		PrivateDependencyModuleNames.Add("FerrealSample");
	}
}
