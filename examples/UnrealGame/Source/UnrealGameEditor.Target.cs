using UnrealBuildTool;

public class UnrealGameEditorTarget : TargetRules
{
	public UnrealGameEditorTarget(TargetInfo Target) : base(Target)
	{
		Type = TargetType.Editor;
		DefaultBuildSettings = BuildSettingsVersion.Latest;
		IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
		ExtraModuleNames.Add("UnrealGame");
	}
}
