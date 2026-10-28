using UnrealBuildTool;

public class FerrealHostEditorTarget : TargetRules
{
	public FerrealHostEditorTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Editor;
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("FerrealHost");
	}
}
