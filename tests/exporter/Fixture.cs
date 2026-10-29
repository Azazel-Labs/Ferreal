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

	private static UhtArrayProperty Array(string name, UhtProperty element, EPropertyFlags flags = EPropertyFlags.None)
	{
		UhtArrayProperty p = Prop<UhtArrayProperty>(name, flags);
		p.ValueProperty = element;
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
	private const EPropertyFlags Public = EPropertyFlags.NativeAccessSpecifierPublic;

	public static int Main(string[] args)
	{
		UhtClass obj = Class("UObject", null);
		UhtFunction ubergraph = Function("ExecuteUbergraph", EFunctionFlags.None, null, Prop<UhtIntProperty>("EntryPoint"));
		ubergraph.MetaData["BlueprintInternalUseOnly"] = "true";
		obj.Functions.Add(ubergraph);

		var collision = new UhtEnum { SourceName = "ECollisionEnabled", EngineName = "ECollisionEnabled", CppForm = UhtEnumCppForm.Namespaced, CppType = "ECollisionEnabled::Type", UnderlyingType = UhtEnumUnderlyingType.Int };
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
			Function("K2_SetActorLocation", Callable, Ret<UhtBoolProperty>(),
				Struct("NewLocation", "FVector"), Prop<UhtBoolProperty>("bSweep"),
				// `FHitResult& SweepHitResult`: an output.
				Struct("SweepHitResult", "FHitResult", EPropertyFlags.OutParm),
				Prop<UhtBoolProperty>("bTeleport")),
			Function("K2_SetActorRotation", Callable, Ret<UhtBoolProperty>(), Struct("NewRotation", "FRotator"), Prop<UhtBoolProperty>("bTeleportPhysics")),
			Function("K2_GetActorLocation", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Struct("ReturnValue", "FVector")),
			Function("GetOwner", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Object("ReturnValue", actor)),
			Function("SetOwner", Callable, null, Object("NewOwner", actor)),
			Function("ActorHasTag", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Ret<UhtBoolProperty>(), Prop<UhtNameProperty>("Tag")),
			Function("GetGameTimeSinceCreation", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Ret<UhtFloatProperty>()),
			// `const FTransform& GetTransform() const` in C++: reflection returns a copy.
			Function("GetTransform", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, Struct("ReturnValue", "FTransform")),
		]);

		UhtClass component = Class("UActorComponent", obj, Function("Activate", Callable, null, Prop<UhtBoolProperty>("bReset")));
		UhtClass scene = Class("USceneComponent", component,
			Function("SetVisibility", Callable, null, Prop<UhtBoolProperty>("bNewVisibility"), Prop<UhtBoolProperty>("bPropagateToChildren")));
		// `namespace ECollisionEnabled { enum Type : int }`, passed as itself: a byte all the same.
		UhtEnumProperty newType = Prop<UhtEnumProperty>("NewType");
		newType.Enum = collision;
		UhtEnumProperty current = Prop<UhtEnumProperty>("ReturnValue");
		current.Enum = collision;
		UhtClass primitive = Class("UPrimitiveComponent", scene,
			Function("SetCollisionEnabled", Callable, null, newType),
			Function("GetCollisionEnabled", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Const, current));

		UhtClass library = Class("UBlueprintFunctionLibrary", obj);
		// Arrays: returned, and passed by const reference (an input, flagged out and ref).
		const EPropertyFlags ConstRef = EPropertyFlags.OutParm | EPropertyFlags.ConstParm | EPropertyFlags.ReferenceParm;
		UhtClass strings = Class("UKismetStringLibrary", library,
			Function("Concat_StrStr", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Ret<UhtStrProperty>(), Prop<UhtStrProperty>("A"), Prop<UhtStrProperty>("B")),
			Function("Conv_IntToString", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Ret<UhtStrProperty>(), Prop<UhtIntProperty>("InInt")),
			Function("ParseIntoArray", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Array("ReturnValue", Prop<UhtStrProperty>("ReturnValue")),
				Prop<UhtStrProperty>("SourceString"), Prop<UhtStrProperty>("Delimiter"), Prop<UhtBoolProperty>("CullEmptyStrings", EPropertyFlags.ConstParm)),
			Function("JoinStringArray", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Ret<UhtStrProperty>(),
				Array("SourceArray", Prop<UhtStrProperty>("SourceArray"), ConstRef), Prop<UhtStrProperty>("Separator")));
		UhtObjectProperty named = Object("Object", obj);
		named.PropertyFlags = EPropertyFlags.ConstParm;
		// `enum ETraceTypeQuery : int` and `namespace EDrawDebugTrace { enum Type : int }`.
		var traceType = new UhtEnum { SourceName = "ETraceTypeQuery", EngineName = "ETraceTypeQuery", CppForm = UhtEnumCppForm.Regular, CppType = "ETraceTypeQuery", UnderlyingType = UhtEnumUnderlyingType.Int };
		traceType.EnumValues.Add(new UhtEnumValue { Name = "TraceTypeQuery1", Value = 0 });
		traceType.EnumValues.Add(new UhtEnumValue { Name = "TraceTypeQuery2", Value = 1 });
		var drawDebug = new UhtEnum { SourceName = "EDrawDebugTrace", EngineName = "EDrawDebugTrace", CppForm = UhtEnumCppForm.Namespaced, CppType = "EDrawDebugTrace::Type", UnderlyingType = UhtEnumUnderlyingType.Int };
		drawDebug.EnumValues.Add(new UhtEnumValue { Name = "EDrawDebugTrace::None", Value = 0 });
		drawDebug.EnumValues.Add(new UhtEnumValue { Name = "EDrawDebugTrace::ForOneFrame", Value = 1 });
		UhtEnumProperty channel = Prop<UhtEnumProperty>("TraceChannel");
		channel.Enum = traceType;
		UhtEnumProperty debugType = Prop<UhtEnumProperty>("DrawDebugType");
		debugType.Enum = drawDebug;
		UhtObjectProperty world = Object("WorldContextObject", obj);
		world.PropertyFlags = EPropertyFlags.ConstParm;
		UhtClass system = Class("UKismetSystemLibrary", library,
			Function("GetDisplayName", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Ret<UhtStrProperty>(), named),
			// As the engine declares it: an array of objects in, an FHitResult out.
			Function("LineTraceSingle", Callable | EFunctionFlags.Static, Ret<UhtBoolProperty>(),
				world, Struct("Start", "FVector", EPropertyFlags.ConstParm), Struct("End", "FVector", EPropertyFlags.ConstParm), channel,
				Prop<UhtBoolProperty>("bTraceComplex"), Array("ActorsToIgnore", Object("ActorsToIgnore", actor), ConstRef), debugType,
				Struct("OutHit", "FHitResult", EPropertyFlags.OutParm), Prop<UhtBoolProperty>("bIgnoreSelf"),
				Struct("TraceColor", "FLinearColor"), Struct("TraceHitColor", "FLinearColor"), Prop<UhtFloatProperty>("DrawTime")));

		// Structs: FVector is a core type, declared to UHT apart from its C++ (NoExport); its
		// network form adds no field; FHitResult's fields are of every kind Ferreal copies, and
		// of kinds it leaves out (private, editor-only, fixed arrays).
		var vector = new UhtScriptStruct { SourceName = "FVector", EngineName = "Vector", ScriptStructFlags = EStructFlags.NoExport };
		// FTransform's members are not public in C++, whatever UHT is told: Ferreal maps it.
		var quat = new UhtScriptStruct { SourceName = "FQuat", EngineName = "Quat", ScriptStructFlags = EStructFlags.NoExport };
		var transform = new UhtScriptStruct { SourceName = "FTransform", EngineName = "Transform", ScriptStructFlags = EStructFlags.NoExport };
		transform.Children.AddRange([Struct("Rotation", "FQuat", Public), Struct("Translation", "FVector", Public), Struct("Scale3D", "FVector", Public)]);
		// A struct with nothing Rust can copy: left out of FHitResult.
		var handle = new UhtScriptStruct { SourceName = "FActorInstanceHandle", EngineName = "ActorInstanceHandle" };
		var referenceObject = Prop<UhtWeakObjectPtrProperty>("ReferenceObject", EPropertyFlags.NativeAccessSpecifierPrivate);
		referenceObject.Class = obj;
		handle.Children.Add(referenceObject);
		UhtStructProperty hitObjectHandle = Struct("HitObjectHandle", "FActorInstanceHandle", Public);
		hitObjectHandle.ScriptStruct = handle;
		var netQuantize = new UhtScriptStruct { SourceName = "FVector_NetQuantize", EngineName = "Vector_NetQuantize", SuperScriptStruct = vector };
		var hit = new UhtScriptStruct { SourceName = "FHitResult", EngineName = "HitResult" };
		hit.MetaData["ToolTip"] = "Structure containing information about one hit of a trace, such as point of impact and surface normal at that point.";
		UhtStructProperty location = Struct("Location", "FVector_NetQuantize", Public);
		location.ScriptStruct = netQuantize;
		var hitComponent = Prop<UhtWeakObjectPtrProperty>("Component", Public);
		hitComponent.Class = primitive;
		var fixedArray = Prop<UhtIntProperty>("Fixed", Public);
		fixedArray.ArrayDimensions = "2";
		hit.Children.AddRange([
			Prop<UhtBoolProperty>("bBlockingHit", Public),
			Prop<UhtFloatProperty>("Time", Public),
			location,
			hitComponent,
			Prop<UhtNameProperty>("BoneName", Public),
			Prop<UhtIntProperty>("FaceIndex", EPropertyFlags.NativeAccessSpecifierPrivate),
			Prop<UhtFloatProperty>("EditorOnlyTime", Public | EPropertyFlags.EditorOnly),
			fixedArray,
			hitObjectHandle,
		]);
		UhtClass physicalMaterial = Class("UPhysicalMaterial", obj);
		// As the engine declares it.
		UhtClass statics = Class("UGameplayStatics", library,
			Function("MakeHitResult", Callable | EFunctionFlags.BlueprintPure | EFunctionFlags.Static, Struct("ReturnValue", "FHitResult"),
				Prop<UhtBoolProperty>("bBlockingHit"), Prop<UhtBoolProperty>("bInitialOverlap"), Prop<UhtFloatProperty>("Time"), Prop<UhtFloatProperty>("Distance"),
				Struct("Location", "FVector"), Struct("ImpactPoint", "FVector"), Struct("Normal", "FVector"), Struct("ImpactNormal", "FVector"),
				Object("PhysMat", physicalMaterial), Object("HitActor", actor), Object("HitComponent", primitive),
				Prop<UhtNameProperty>("HitBoneName"), Prop<UhtNameProperty>("BoneName"),
				Prop<UhtIntProperty>("HitItem"), Prop<UhtIntProperty>("ElementIndex"), Prop<UhtIntProperty>("FaceIndex"),
				Struct("TraceStart", "FVector"), Struct("TraceEnd", "FVector")));

		// `UPARAM(ref) FVector& A`: a reference read and changed.
		UhtClass math = Class("UKismetMathLibrary", library,
			Function("Vector_Normalize", Callable | EFunctionFlags.Static, null,
				Struct("A", "FVector", EPropertyFlags.OutParm | EPropertyFlags.ReferenceParm), Prop<UhtFloatProperty>("Tolerance")));

		// A class of the project (Rust's own: not bound).
		UhtClass greeter = Class("UGreeter", obj, Function("Greet", Callable, Ret<UhtStrProperty>(), Prop<UhtStrProperty>("Punctuation")));

		static UhtHeaderFile Header(string include, params UhtType[] types)
		{
			var h = new UhtHeaderFile { IncludeFilePath = include };
			h.Children.AddRange(types);
			return h;
		}
		var session = new UhtSession();
		session.Modules.Add(new UhtModule { ShortName = "CoreUObject", Headers = [Header("UObject/Object.h", obj), Header("UObject/NoExportTypes.h", vector, quat, transform)] });
		session.Modules.Add(new UhtModule
		{
			ShortName = "Engine",
			Headers =
			[
				Header("GameFramework/Actor.h", actor),
				Header("Components/ActorComponent.h", component),
				Header("Components/SceneComponent.h", scene),
				Header("Components/PrimitiveComponent.h", primitive),
				Header("Engine/EngineTypes.h", collision, traceType),
				Header("Kismet/BlueprintFunctionLibrary.h", library),
				Header("Kismet/KismetStringLibrary.h", strings),
				Header("Kismet/KismetSystemLibrary.h", drawDebug, system),
				Header("Kismet/KismetMathLibrary.h", math),
				Header("Engine/NetSerialization.h", netQuantize),
				Header("Engine/ActorInstanceHandle.h", handle),
				Header("Engine/HitResult.h", hit),
				Header("PhysicalMaterials/PhysicalMaterial.h", physicalMaterial),
				Header("Kismet/GameplayStatics.h", statics),
			],
		});
		session.Modules.Add(new UhtModule { ShortName = "HelloRust", Headers = [Header("Ferreal/Greeter.h", greeter)] });

		var factory = new Factory(session);
		FerrealExporter.Export(factory);
		File.WriteAllText(args.Length > 0 ? args[0] : "FerrealEngine.json", factory.Output);
		return 0;
	}
}
