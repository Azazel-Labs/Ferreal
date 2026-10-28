// A made-up session of Unreal Header Tool: a few classes in the shape of the engine's (their
// reflected functions as the engine declares them), run through the Ferreal exporter. The mock
// engine (../mock-ue) registers the same functions, so the test calls them end to end.

using System.Text;
using EpicGames.Core;
using EpicGames.UHT.Types;
using EpicGames.UHT.Utils;

namespace Ferreal.Tests;

public static class Fixture
{
	private sealed class Factory(UhtSession session) : IUhtExportFactory
	{
		public UhtSession Session { get; } = session;
		public string Output = "";
		public string MakePath(string name, string extension) => name + extension;
		public void CommitOutput(string path, StringBuilder contents) => Output = contents.ToString();
	}

	private static T Prop<T>(string name, EPropertyFlags flags = EPropertyFlags.None) where T : UhtProperty, new()
		=> new T { SourceName = name, EngineName = name, PropertyFlags = flags };

	private static UhtProperty Ret<T>() where T : UhtProperty, new() => Prop<T>("ReturnValue");

	private static UhtStructProperty Struct(string name, string structName, EPropertyFlags flags = EPropertyFlags.None)
	{
		UhtStructProperty p = Prop<UhtStructProperty>(name, flags);
		p.ScriptStruct = new UhtScriptStruct { SourceName = structName, EngineName = structName[1..] };
		return p;
	}

	private static UhtObjectProperty Object(string name, UhtClass c)
	{
		UhtObjectProperty p = Prop<UhtObjectProperty>(name);
		p.Class = c;
		return p;
	}

	private static UhtFunction Function(string name, EFunctionFlags flags, UhtProperty ret, params UhtProperty[] parms)
	{
		var f = new UhtFunction
		{
			SourceName = name,
			EngineName = name,
			FunctionType = UhtFunctionType.Function,
			FunctionFlags = EFunctionFlags.Native | EFunctionFlags.Public | flags,
			ParameterProperties = parms,
			ReturnProperty = ret,
		};
		return f;
	}

	private static UhtClass Class(string name, UhtClass super, params UhtFunction[] functions)
	{
		var c = new UhtClass { SourceName = name, EngineName = name[1..], ClassType = UhtClassType.Class, SuperClass = super };
		c.Functions.AddRange(functions);
		return c;
	}

	private const EFunctionFlags Callable = EFunctionFlags.BlueprintCallable;

	public static int Main(string[] args)
	{
		UhtClass obj = Class("UObject", null);
		UhtFunction ubergraph = Function("ExecuteUbergraph", EFunctionFlags.None, null, Prop<UhtIntProperty>("EntryPoint"));
		ubergraph.MetaData["BlueprintInternalUseOnly"] = "true";
		obj.Functions.Add(ubergraph);

		var collision = new UhtEnum { SourceName = "ECollisionEnabled", EngineName = "ECollisionEnabled", UnderlyingType = UhtEnumUnderlyingType.Unspecified };
		collision.MetaData["ToolTip"] = "Enum used to describe what type of collision is enabled on a body.";
		string[] modes = ["NoCollision", "QueryOnly", "PhysicsOnly", "QueryAndPhysics", "ProbeOnly", "QueryAndProbe"];
		for (int i = 0; i < modes.Length; ++i)
		{
			collision.EnumValues.Add(new UhtEnumValue { Name = "ECollisionEnabled::" + modes[i], Value = i });
		}
		collision.EnumValues.Add(new UhtEnumValue { Name = "ECollisionEnabled_MAX", Value = 6 });
		collision.EnumValues.Add(new UhtEnumValue { Name = "ECollisionEnabled::Computed", Value = -1 });

		UhtClass actor = Class("AActor", obj);
		actor.MetaData["ToolTip"] = "Actor is the base class for an Object that can be placed or spawned in a level.";
		UhtFunction hide = Function("SetActorHiddenInGame", Callable, null, Prop<UhtBoolProperty>("bNewHidden"));
		hide.MetaData["ToolTip"] = "Sets the actor to be hidden in the game\n    @param bNewHidden Whether or not to hide the actor and all its components";
		actor.Functions.AddRange([
			hide,
			Function("GetActorNameOrLabel", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Ret<UhtStrProperty>()),
			Function("K2_SetActorLocation", Callable, Ret<UhtBoolProperty>(),
				Struct("NewLocation", "FVector"), Prop<UhtBoolProperty>("bSweep"),
				Struct("SweepHitResult", "FHitResult", EPropertyFlags.OutParm | EPropertyFlags.ReferenceParm),
				Prop<UhtBoolProperty>("bTeleport")),
			Function("K2_SetActorRotation", Callable, Ret<UhtBoolProperty>(), Struct("NewRotation", "FRotator"), Prop<UhtBoolProperty>("bTeleportPhysics")),
			Function("K2_GetActorLocation", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Struct("ReturnValue", "FVector")),
			Function("GetOwner", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Object("ReturnValue", actor)),
			Function("SetOwner", Callable, null, Object("NewOwner", actor)),
			Function("ActorHasTag", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Ret<UhtBoolProperty>(), Prop<UhtNameProperty>("Tag")),
			Function("GetGameTimeSinceCreation", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Ret<UhtFloatProperty>()),
		]);

		UhtClass component = Class("UActorComponent", obj, Function("Activate", Callable, null, Prop<UhtBoolProperty>("bReset")));
		UhtClass scene = Class("USceneComponent", component,
			Function("SetVisibility", Callable, null, Prop<UhtBoolProperty>("bNewVisibility"), Prop<UhtBoolProperty>("bPropagateToChildren")));
		UhtByteProperty newType = Prop<UhtByteProperty>("NewType");
		newType.Enum = collision;
		UhtByteProperty current = Prop<UhtByteProperty>("ReturnValue");
		current.Enum = collision;
		UhtClass primitive = Class("UPrimitiveComponent", scene,
			Function("SetCollisionEnabled", Callable, null, newType),
			Function("GetCollisionEnabled", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, current));

		UhtClass library = Class("UBlueprintFunctionLibrary", obj);
		UhtClass strings = Class("UKismetStringLibrary", library,
			Function("Concat_StrStr", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Ret<UhtStrProperty>(), Prop<UhtStrProperty>("A"), Prop<UhtStrProperty>("B")),
			Function("Conv_IntToString", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Ret<UhtStrProperty>(), Prop<UhtIntProperty>("InInt")));

		// A class of the project (Rust's own: not bound).
		UhtClass greeter = Class("UGreeter", obj, Function("Greet", Callable, Ret<UhtStrProperty>(), Prop<UhtStrProperty>("Punctuation")));

		static UhtHeaderFile Header(string include, params UhtType[] types)
		{
			var h = new UhtHeaderFile { IncludeFilePath = include };
			h.Children.AddRange(types);
			return h;
		}
		var session = new UhtSession();
		session.Modules.Add(new UhtModule { ShortName = "CoreUObject", Headers = [Header("UObject/Object.h", obj)] });
		session.Modules.Add(new UhtModule
		{
			ShortName = "Engine",
			Headers =
			[
				Header("GameFramework/Actor.h", actor),
				Header("Components/ActorComponent.h", component),
				Header("Components/SceneComponent.h", scene),
				Header("Components/PrimitiveComponent.h", primitive),
				Header("Engine/EngineTypes.h", collision),
				Header("Kismet/BlueprintFunctionLibrary.h", library),
				Header("Kismet/KismetStringLibrary.h", strings),
			],
		});
		session.Modules.Add(new UhtModule { ShortName = "HelloRust", Headers = [Header("Ferreal/Greeter.h", greeter)] });

		var factory = new Factory(session);
		FerrealExporter.Export(factory);
		File.WriteAllText(args.Length > 0 ? args[0] : "FerrealEngine.json", factory.Output);
		return 0;
	}
}
