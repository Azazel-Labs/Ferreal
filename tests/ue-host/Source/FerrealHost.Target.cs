using UnrealBuildTool;

public class FerrealHostTarget : TargetRules
{
	public FerrealHostTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Game;
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("FerrealHost");
	}
}
