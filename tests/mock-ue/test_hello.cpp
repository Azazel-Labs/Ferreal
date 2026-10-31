// Drives the example's UGreeter (Rust) through the generated C++ glue, in the mock engine.

#include "Ferreal/Greeter.h"
#include "Ferreal/FerrealHelloRust.h"
#include "Components/PrimitiveComponent.h"
#include "Engine/HitResult.h"
#include "Misc/DateTime.h"
#include "Misc/Timespan.h"
#include "Engine/Texture2D.h"
#include "GameFramework/Actor.h"

#include <cmath>
#include <cstdlib>

static int Failures = 0;

/** "\u00dcn\u00efc\u00f8d\u00e9 \U0001F600" in UTF-8. */
static const std::string Unicode = "\xc3\x9c" "n" "\xc3\xaf" "c" "\xc3\xb8" "d" "\xc3\xa9" " " "\xf0\x9f\x98\x80";

#define EXPECT(Cond) do { if (!(Cond)) { std::printf("FAILED line %d: %s\n", __LINE__, #Cond); ++Failures; } } while (0)

// The mock engine's reflected functions: the classes and functions of FerrealEngine.json (the
// exporter's output for tests/exporter/Fixture.cs), registered the way the engine links
// them. The natives read and write their parameters through the properties' offsets, as
// the engine's thunks do, not through the structs Ferreal generates.

struct FActorState
{
	bool bHidden = false;
	UObject* Owner = nullptr;
	FRotator Rotation;
	FVector Location{1.0, 2.0, 3.0};
	bool bVisible = true;
	bool bPropagate = false;
	int32 Collision = 0;
};
static FActorState State;

static FProperty* Param(const char* Name, const char* Class, int32 Size, int32 Align)
{
	return new FProperty(Name, Class, Size, Align, CPF_Parm);
}

static FProperty* Return(const char* Class, int32 Size, int32 Align)
{
	return new FProperty("ReturnValue", Class, Size, Align, CPF_Parm | CPF_ReturnParm);
}

/** A TArray<T> parameter whose elements are Inner (`flags`: CPF_ReturnParm for the return value). */
template <typename T>
static FProperty* ArrayParam(const char* Name, FProperty* Inner, uint64 Flags = 0)
{
	return new FArrayProperty(Name, Inner, sizeof(TArray<T>), alignof(TArray<T>), CPF_Parm | Flags);
}

template <typename T>
static FProperty* OutParam(const char* Name, UScriptStruct* Struct)
{
	return new FStructProperty(Name, Struct, sizeof(T), alignof(T), CPF_Parm | CPF_OutParm);
}

template <typename T>
static FProperty* StructParam(const char* Name, UScriptStruct* Struct, bool bReturn = false)
{
	return new FStructProperty(Name, Struct, sizeof(T), alignof(T), CPF_Parm | (bReturn ? uint64(CPF_ReturnParm) : uint64(0)));
}

static void AddFunction(UClass& Class, const char* Name, std::vector<FProperty*> Params, std::function<void(UFunction&, UObject*, uint8*)> Native)
{
	auto F = std::make_unique<UFunction>();
	F->Name = Name;
	for (FProperty* P : Params)
	{
		F->Properties.emplace_back(P);
	}
	F->Link();
	UFunction* Raw = F.get();
	F->Native = [Raw, Native](UObject* Self, uint8* Parms) { Native(*Raw, Self, Parms); };
	Class.Functions[Name] = std::move(F);
}

struct FMockEngine
{
	UScriptStruct Vector{"Vector"};
	UScriptStruct Rotator{"Rotator"};
	UScriptStruct HitResult{"HitResult"};
	UScriptStruct Transform{"Transform"};
	UScriptStruct LinearColor{"LinearColor"};
	UScriptStruct IntPoint{"IntPoint"};
	UScriptStruct IntVector{"IntVector"};
	UScriptStruct Vector4{"Vector4"};
	UScriptStruct Color{"Color"};
	UScriptStruct Box{"Box"};
	UScriptStruct Matrix{"Matrix"};
	UScriptStruct DateTime{"DateTime"};
	UScriptStruct Timespan{"Timespan"};
	UClass Object{"Object", nullptr};
	UClass Actor{"Actor", &Object};
	UClass ActorComponent{"ActorComponent", &Object};
	UClass SceneComponent{"SceneComponent", &ActorComponent};
	UClass PrimitiveComponent{"PrimitiveComponent", &SceneComponent};
	UClass Library{"BlueprintFunctionLibrary", &Object};
	UClass Strings{"KismetStringLibrary", &Library};
	UObject StringsDefault;
	UClass System{"KismetSystemLibrary", &Library};
	UObject SystemDefault;
	UClass Statics{"GameplayStatics", &Library};
	UObject StaticsDefault;
	UClass Math{"KismetMathLibrary", &Library};
	UObject MathDefault;

	FMockEngine()
	{
		FerrealMockClasses()[u"/Script/CoreUObject.Object"] = &Object;
		FerrealMockClasses()[u"/Script/Engine.Actor"] = &Actor;
		FerrealMockClasses()[u"/Script/Engine.ActorComponent"] = &ActorComponent;
		FerrealMockClasses()[u"/Script/Engine.SceneComponent"] = &SceneComponent;
		FerrealMockClasses()[u"/Script/Engine.PrimitiveComponent"] = &PrimitiveComponent;
		FerrealMockClasses()[u"/Script/Engine.BlueprintFunctionLibrary"] = &Library;
		FerrealMockClasses()[u"/Script/Engine.KismetStringLibrary"] = &Strings;
		FerrealMockClasses()[u"/Script/Engine.KismetSystemLibrary"] = &System;
		StringsDefault.MockClass = &Strings;
		Strings.DefaultObject = &StringsDefault;
		SystemDefault.MockClass = &System;
		System.DefaultObject = &SystemDefault;
		FerrealMockClasses()[u"/Script/Engine.GameplayStatics"] = &Statics;
		StaticsDefault.MockClass = &Statics;
		Statics.DefaultObject = &StaticsDefault;
		FerrealMockClasses()[u"/Script/Engine.KismetMathLibrary"] = &Math;
		MathDefault.MockClass = &Math;
		Math.DefaultObject = &MathDefault;

		const int32 Str = sizeof(FString), StrAlign = alignof(FString);
		const int32 Ptr = sizeof(UObject*);
		AddFunction(Actor, "SetActorHiddenInGame", {Param("bNewHidden", "BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { State.bHidden = F.Param<bool>(P, 0); });
		// An output (FHitResult& SweepHitResult), filled when sweeping.
		AddFunction(Actor, "K2_SetActorLocation",
			{StructParam<FVector>("NewLocation", &Vector), Param("bSweep", "BoolProperty", 1, 1), OutParam<FHitResult>("SweepHitResult", &HitResult), Param("bTeleport", "BoolProperty", 1, 1), Return("BoolProperty", 1, 1)},
			[](UFunction& F, UObject* Self, uint8* P)
			{
				if (F.Param<bool>(P, 1))
				{
					FHitResult& Hit = F.Param<FHitResult>(P, 2);
					Hit.Time = 0.25f;
					Hit.HitObjectHandle = FActorInstanceHandle(static_cast<AActor*>(Self));
				}
				F.Param<bool>(P, 4) = true;
			});
		AddFunction(Actor, "K2_SetActorRotation",
			{StructParam<FRotator>("NewRotation", &Rotator), Param("bTeleportPhysics", "BoolProperty", 1, 1), Return("BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { State.Rotation = F.Param<FRotator>(P, 0); F.Param<bool>(P, 2) = !F.Param<bool>(P, 1); });
		AddFunction(Actor, "K2_GetActorLocation", {StructParam<FVector>("ReturnValue", &Vector, true)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FVector>(P, 0) = State.Location; });
		AddFunction(Actor, "GetOwner", {Return("ObjectProperty", Ptr, Ptr)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<UObject*>(P, 0) = State.Owner; });
		AddFunction(Actor, "SetOwner", {Param("NewOwner", "ObjectProperty", Ptr, Ptr)},
			[](UFunction& F, UObject*, uint8* P) { State.Owner = F.Param<UObject*>(P, 0); });
		AddFunction(Actor, "ActorHasTag", {Param("Tag", "NameProperty", sizeof(FName), alignof(FName)), Return("BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<bool>(P, 1) = F.Param<FName>(P, 0) == FName("Rusty"); });
		// The engine says double, the bindings were generated for float: the call must be refused.
		AddFunction(Actor, "GetGameTimeSinceCreation", {Return("DoubleProperty", 8, 8)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<double>(P, 0) = 42.0; });
		AddFunction(ActorComponent, "Activate", {Param("bReset", "BoolProperty", 1, 1)}, [](UFunction&, UObject*, uint8*) {});
		AddFunction(SceneComponent, "SetVisibility", {Param("bNewVisibility", "BoolProperty", 1, 1), Param("bPropagateToChildren", "BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { State.bVisible = F.Param<bool>(P, 0); State.bPropagate = F.Param<bool>(P, 1); });
		// ECollisionEnabled::Type is an `enum Type : int` passed as itself, which reflection
		// holds in a byte (only an enum class is an EnumProperty).
		AddFunction(PrimitiveComponent, "SetCollisionEnabled", {Param("NewType", "ByteProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { State.Collision = F.Param<uint8>(P, 0); });
		AddFunction(PrimitiveComponent, "GetCollisionEnabled", {Return("ByteProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<uint8>(P, 0) = static_cast<uint8>(State.Collision); });
		AddFunction(Strings, "Concat_StrStr", {Param("A", "StrProperty", Str, StrAlign), Param("B", "StrProperty", Str, StrAlign), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject* Self, uint8* P) { F.Param<FString>(P, 2) = FString(F.Param<FString>(P, 0).Str + F.Param<FString>(P, 1).Str); (void)Self; });
		AddFunction(Strings, "Conv_IntToString", {Param("InInt", "IntProperty", 4, 4), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FString>(P, 1) = FString::Printf(u"%d", F.Param<int32>(P, 0)); });
		// Structs: returned by value (a copy for `const FTransform&`), copied to Rust field by field.
		AddFunction(Actor, "GetTransform", {StructParam<FTransform>("ReturnValue", &Transform, true)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FTransform>(P, 0) = FTransform(FQuat(0, 0, 0.5, 1), State.Location, FVector(2, 2, 2)); });
		const int32 Name = sizeof(FName), NameAlign = alignof(FName);
		AddFunction(Statics, "MakeHitResult",
			{Param("bBlockingHit", "BoolProperty", 1, 1), Param("bInitialOverlap", "BoolProperty", 1, 1), Param("Time", "FloatProperty", 4, 4), Param("Distance", "FloatProperty", 4, 4),
				StructParam<FVector>("Location", &Vector), StructParam<FVector>("ImpactPoint", &Vector), StructParam<FVector>("Normal", &Vector), StructParam<FVector>("ImpactNormal", &Vector),
				Param("PhysMat", "ObjectProperty", Ptr, Ptr), Param("HitActor", "ObjectProperty", Ptr, Ptr), Param("HitComponent", "ObjectProperty", Ptr, Ptr),
				Param("HitBoneName", "NameProperty", Name, NameAlign), Param("BoneName", "NameProperty", Name, NameAlign),
				Param("HitItem", "IntProperty", 4, 4), Param("ElementIndex", "IntProperty", 4, 4), Param("FaceIndex", "IntProperty", 4, 4),
				StructParam<FVector>("TraceStart", &Vector), StructParam<FVector>("TraceEnd", &Vector), StructParam<FHitResult>("ReturnValue", &HitResult, true)},
			[](UFunction& F, UObject*, uint8* P)
			{
				FHitResult& Hit = F.Param<FHitResult>(P, 18);
				Hit.bBlockingHit = F.Param<bool>(P, 0);
				Hit.Time = F.Param<float>(P, 2);
				Hit.Location = F.Param<FVector>(P, 4);
				Hit.Component = Cast<UPrimitiveComponent>(F.Param<UObject*>(P, 10));
				Hit.BoneName = F.Param<FName>(P, 11);
				Hit.HitObjectHandle = FActorInstanceHandle(Cast<AActor>(F.Param<UObject*>(P, 9)));
			});
		// A reference read and changed (UPARAM(ref) FVector& A).
		AddFunction(Math, "Vector_Normalize", {OutParam<FVector>("A", &Vector), Param("Tolerance", "FloatProperty", 4, 4)},
			[](UFunction& F, UObject*, uint8* P)
			{
				FVector& A = F.Param<FVector>(P, 0);
				const double Length = std::sqrt(A.X * A.X + A.Y * A.Y + A.Z * A.Z);
				A = FVector(A.X / Length, A.Y / Length, A.Z / Length);
			});
		// Arrays: returned (to a sink), passed in (from a slice).
		AddFunction(Strings, "ParseIntoArray",
			{Param("SourceString", "StrProperty", Str, StrAlign), Param("Delimiter", "StrProperty", Str, StrAlign), Param("CullEmptyStrings", "BoolProperty", 1, 1),
				ArrayParam<FString>("ReturnValue", Param("ReturnValue", "StrProperty", Str, StrAlign), CPF_ReturnParm)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FString>(P, 0).ParseIntoArray(F.Param<TArray<FString>>(P, 3), *F.Param<FString>(P, 1)); });
		AddFunction(Strings, "JoinStringArray",
			{ArrayParam<FString>("SourceArray", Param("SourceArray", "StrProperty", Str, StrAlign), CPF_OutParm), Param("Separator", "StrProperty", Str, StrAlign), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject*, uint8* P)
			{
				std::u16string Out;
				for (const FString& S : F.Param<TArray<FString>>(P, 0))
				{
					Out += (Out.empty() ? u"" : F.Param<FString>(P, 1).Str) + S.Str;
				}
				F.Param<FString>(P, 2) = FString(Out);
			});
		// Hits the world context object, unless it is ignored.
		AddFunction(System, "LineTraceSingle",
			{Param("WorldContextObject", "ObjectProperty", Ptr, Ptr), StructParam<FVector>("Start", &Vector), StructParam<FVector>("End", &Vector), Param("TraceChannel", "ByteProperty", 1, 1),
				Param("bTraceComplex", "BoolProperty", 1, 1), ArrayParam<UObject*>("ActorsToIgnore", Param("ActorsToIgnore", "ObjectProperty", Ptr, Ptr), CPF_OutParm),
				Param("DrawDebugType", "ByteProperty", 1, 1), OutParam<FHitResult>("OutHit", &HitResult), Param("bIgnoreSelf", "BoolProperty", 1, 1),
				StructParam<FLinearColor>("TraceColor", &LinearColor), StructParam<FLinearColor>("TraceHitColor", &LinearColor), Param("DrawTime", "FloatProperty", 4, 4), Return("BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P)
			{
				UObject* Target = F.Param<UObject*>(P, 0);
				if (F.Param<TArray<UObject*>>(P, 5).Contains(Target))
				{
					return;
				}
				FHitResult& Hit = F.Param<FHitResult>(P, 7);
				Hit.bBlockingHit = true;
				Hit.Location = F.Param<FVector>(P, 2);
				Hit.HitObjectHandle = FActorInstanceHandle(Cast<AActor>(Target));
				F.Param<bool>(P, 12) = true;
			});
		// Core types (NoExport), as the engine's functions treat them.
		AddFunction(Math, "Add_IntPointIntPoint", {StructParam<FIntPoint>("A", &IntPoint), StructParam<FIntPoint>("B", &IntPoint), StructParam<FIntPoint>("ReturnValue", &IntPoint, true)},
			[](UFunction& F, UObject*, uint8* P)
			{
				const FIntPoint& A = F.Param<FIntPoint>(P, 0);
				const FIntPoint& B = F.Param<FIntPoint>(P, 1);
				F.Param<FIntPoint>(P, 2) = FIntPoint(A.X + B.X, A.Y + B.Y);
			});
		AddFunction(Math, "Conv_IntToIntVector", {Param("InInt", "IntProperty", 4, 4), StructParam<FIntVector>("ReturnValue", &IntVector, true)},
			[](UFunction& F, UObject*, uint8* P) { const int32 I = F.Param<int32>(P, 0); F.Param<FIntVector>(P, 1) = FIntVector(I, I, I); });
		AddFunction(Math, "Conv_Vector4ToVector", {StructParam<FVector4>("InVector4", &Vector4), StructParam<FVector>("ReturnValue", &Vector, true)},
			[](UFunction& F, UObject*, uint8* P) { const FVector4& V = F.Param<FVector4>(P, 0); F.Param<FVector>(P, 1) = FVector(V.X, V.Y, V.Z); });
		AddFunction(Math, "Conv_LinearColorToColor", {StructParam<FLinearColor>("InLinearColor", &LinearColor), Param("InUseSRGB", "BoolProperty", 1, 1), StructParam<FColor>("ReturnValue", &Color, true)},
			[](UFunction& F, UObject*, uint8* P)
			{
				const FLinearColor& C = F.Param<FLinearColor>(P, 0);
				auto Byte = [](float V) { return static_cast<uint8>(V * 255.0f + 0.5f); };
				F.Param<FColor>(P, 2) = FColor(Byte(C.R), Byte(C.G), Byte(C.B), Byte(C.A));
			});
		AddFunction(Math, "MakeBox", {StructParam<FVector>("Min", &Vector), StructParam<FVector>("Max", &Vector), StructParam<FBox>("ReturnValue", &Box, true)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FBox>(P, 2) = FBox(F.Param<FVector>(P, 0), F.Param<FVector>(P, 1)); });
		AddFunction(Math, "Matrix_GetOrigin", {StructParam<FMatrix>("InMatrix", &Matrix), StructParam<FVector>("ReturnValue", &Vector, true)},
			[](UFunction& F, UObject*, uint8* P) { const FMatrix& M = F.Param<FMatrix>(P, 0); F.Param<FVector>(P, 1) = FVector(M.M[3][0], M.M[3][1], M.M[3][2]); });
		// Days from 0001-01-01 of the first of a month, enough for the test's dates.
		AddFunction(Math, "MakeDateTime",
			{Param("Year", "IntProperty", 4, 4), Param("Month", "IntProperty", 4, 4), Param("Day", "IntProperty", 4, 4), Param("Hour", "IntProperty", 4, 4),
				Param("Minute", "IntProperty", 4, 4), Param("Second", "IntProperty", 4, 4), Param("Millisecond", "IntProperty", 4, 4), StructParam<FDateTime>("ReturnValue", &DateTime, true)},
			[](UFunction& F, UObject*, uint8* P)
			{
				const int64 Days = (F.Param<int32>(P, 0) - 1) * int64(365) + F.Param<int32>(P, 2) - 1;
				const int64 Seconds = ((Days * 24 + F.Param<int32>(P, 3)) * 60 + F.Param<int32>(P, 4)) * 60 + F.Param<int32>(P, 5);
				F.Param<FDateTime>(P, 7) = FDateTime(Seconds * 10000000);
			});
		AddFunction(Math, "Add_DateTimeTimespan", {StructParam<FDateTime>("A", &DateTime), StructParam<FTimespan>("B", &Timespan), StructParam<FDateTime>("ReturnValue", &DateTime, true)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FDateTime>(P, 2) = FDateTime(F.Param<FDateTime>(P, 0).GetTicks() + F.Param<FTimespan>(P, 1).GetTicks()); });
		AddFunction(Math, "GetYear", {StructParam<FDateTime>("A", &DateTime), Return("IntProperty", 4, 4)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<int32>(P, 1) = static_cast<int32>(F.Param<FDateTime>(P, 0).GetTicks() / 10000000 / 86400 / 365) + 1; });
		AddFunction(Math, "FromSeconds", {Param("Seconds", "DoubleProperty", 8, 8), StructParam<FTimespan>("ReturnValue", &Timespan, true)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FTimespan>(P, 1) = FTimespan(static_cast<int64>(F.Param<double>(P, 0) * 10000000)); });
		// Class references (UClass*, TSubclassOf): the world holds the actor passed, alone.
		AddFunction(Statics, "GetObjectClass", {Param("Object", "ObjectProperty", Ptr, Ptr), Return("ClassProperty", Ptr, Ptr)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<UClass*>(P, 1) = F.Param<UObject*>(P, 0)->GetClass(); });
		AddFunction(Statics, "GetAllActorsOfClass",
			{Param("WorldContextObject", "ObjectProperty", Ptr, Ptr), Param("ActorClass", "ClassProperty", Ptr, Ptr),
				ArrayParam<UObject*>("OutActors", Param("OutActors", "ObjectProperty", Ptr, Ptr), CPF_OutParm)},
			[](UFunction& F, UObject*, uint8* P)
			{
				UObject* World = F.Param<UObject*>(P, 0);
				if (World->IsA(F.Param<UClass*>(P, 1))) F.Param<TArray<UObject*>>(P, 2).Add(World);
			});
		AddFunction(Math, "ClassIsChildOf", {Param("TestClass", "ClassProperty", Ptr, Ptr), Param("ParentClass", "ClassProperty", Ptr, Ptr), Return("BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<bool>(P, 2) = F.Param<UClass*>(P, 0)->IsChildOf(F.Param<UClass*>(P, 1)); });
		AddFunction(System, "GetClassDisplayName", {Param("Class", "ClassProperty", Ptr, Ptr), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FString>(P, 1) = F.Param<UClass*>(P, 0)->GetName(); });
		AddFunction(System, "GetDisplayName", {Param("Object", "ObjectProperty", Ptr, Ptr), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FString>(P, 1) = FString(F.Param<UObject*>(P, 0) ? u"Rusty actor" : u"None"); });
	}
};

static bool Logged(const std::string& Text)
{
	for (const std::string& L : GFerrealMockLogs)
	{
		if (L.find(Text) != std::string::npos) return true;
	}
	return false;
}

int main()
{
	FMockEngine Engine;

	UGreeter* G = new UGreeter(FObjectInitializer());

	// Defaults from #[uproperty(default = ...)], #[init], strings and setters.
	EXPECT(ToUtf8(G->Name) == "World");
	EXPECT(G->Scale == 2.5f && G->bLoud && G->Mood == EMood::Excited);
	EXPECT(G->Tint.R == 1.0f && G->Tint.G == 0.5f && G->Tint.A == 1.0f);
	EXPECT(ToUtf8(G->Greet(TEXT("!"))) == "HELLO, WORLD! #101");
	G->SetLoud(false);
	EXPECT(ToUtf8(G->Greet(TEXT("?"))) == "Hello, World? #102");
	G->SetName(FString(u"Ünïcødé \U0001F600"));
	EXPECT(ToUtf8(G->Greet(TEXT("."))) == std::string("Hello, ") + Unicode + ". #103");
	EXPECT(G->ChangeCount() == 2);

	// Values, static functions, enums, colors.
	const FVector2D S = G->Size(FVector2D(1.0, 2.0));
	EXPECT(S.X == 3.5 && S.Y == 7.0);
	EXPECT(UGreeter::Add(2, 3) == 5);
	EXPECT(G->NextMood() == EMood::Sleepy && G->Mood == EMood::Sleepy);
	const FLinearColor C = G->Brighter(0.25f);
	EXPECT(C.R == 1.25f && C.G == 0.75f && C.B == 0.25f);

	// Objects and arrays.
	UTexture2D* T1 = new UTexture2D();
	UTexture2D* T2 = new UTexture2D();
	G->Textures.Add(T1);
	G->Textures.Add(T2);
	G->Texture = T2;
	EXPECT(G->CountTextures() == 3);
	G->KeepFirstTexture();
	EXPECT(G->Textures.Num() == 1 && G->Textures[0].Get() == T1 && G->Texture.Get() == T1);
	EXPECT(G->Sum(TArray<float>{1.0f, 2.0f, 3.5f}) == 6.5f);

	// Arrays both ways.
	const TArray<float> Doubled = G->Doubled();
	EXPECT(Doubled.Num() == 3 && Doubled[0] == 2.0f && Doubled[2] == 500.0f);

	// Rust calling the C++ parent (#[cpp_base]), overrides calling Super first.
	EXPECT(G->Ping() == 3);
	EXPECT(ToUtf8(G->LastMessage) == "ping from the base" && G->LastSum == 6.5f);
	G->OnTick(0.5f);
	EXPECT(G->Ticks == 1 && G->Scale == 3.0f);
	EXPECT(ToUtf8(G->Describe()) == "103 greetings, mood Sleepy");

	// The editor's change notifications.
	FPropertyChangedEvent Event{FName("Scale")};
	G->PostEditChangeProperty(Event);
	EXPECT(G->ChangeCount() == 3);

	// A panic is caught at the boundary and logged.
	EXPECT(G->Panics() == 0);
	EXPECT(ToUtf8(GFerrealMockLastLog).find("on purpose") != std::string::npos);
	EXPECT(ToUtf8(G->Greet(TEXT("!"))) == std::string("Hello, ") + Unicode + "! #104");

	// Exported functions, with an object handle.
	EXPECT(ToUtf8(Ferreal::HelloRust::Shout(TEXT("hey"), G)) == "HEY");
	EXPECT(ToUtf8(G->LastMessage) == "HEY" && G->LastSum == 0.0f);
	EXPECT(ToUtf8(Ferreal::HelloRust::Shout(TEXT("quiet"), nullptr)) == "QUIET");

	// Engine functions through the generated bindings (ferreal::engine!): reflected calls with
	// their parameters checked against the engine's reflection first.
	AActor* Actor = new AActor();
	Actor->MockClass = &Engine.Actor;
	UPrimitiveComponent* Component = new UPrimitiveComponent();
	Component->MockClass = &Engine.PrimitiveComponent;
	State.Owner = Component;
	const std::string Report = ToUtf8(Ferreal::HelloRust::Poke(Actor, Component));
	EXPECT(Report == "Rusty actor own=true turned=true z=3 tagged=true collision=3 age=0");
	EXPECT(State.bHidden && State.Owner == nullptr && State.Rotation.Yaw == 20.0 && State.Rotation.Roll == 30.0);
	EXPECT(!State.bVisible && State.bPropagate && State.Collision == 3);
	EXPECT(Logged("/Script/Engine.Actor.GetGameTimeSinceCreation cannot be called from Rust"));
	// C++ written inline in the Rust (cpp!): a plain method, FMath, a string passed both ways.
	EXPECT(ToUtf8(Ferreal::HelloRust::Label(Actor, 1)) == "Rusty actor x2");
	EXPECT(ToUtf8(Ferreal::HelloRust::Label(Actor, 5)) == "Rusty actor x5");
	// Engine structs copied to Rust (FHitResult field by field, FTransform through its accessors).
	EXPECT(ToUtf8(Ferreal::HelloRust::Hit(Actor, Component)) == "blocking=true time=0.5 z=3 component=true actor=true bone=Spine scale=2 turn=0.5 later=1.5 Spine moved=13 next=2");
	// Outputs: an FHitResult the engine fills, an FVector it reads and changes.
	EXPECT(ToUtf8(Ferreal::HelloRust::Sweep(Actor)) == "moved=true swept=0.25 actor=true x=0.600 shared=true");
	// Arrays: of strings returned and passed, of objects passed with an FHitResult out.
	EXPECT(ToUtf8(Ferreal::HelloRust::Trace(Actor)) == "parts=[\"a\", \"b\", \"c\"] joined=x+y hit=true z=-100 actor=true ignored=false");
	// Core types (NoExport), mapped to Ferreal's own.
	EXPECT(ToUtf8(Ferreal::HelloRust::CoreTypes()) == "point=4,6 cube=7 flat=3 color=255,0,0,255 box=true,3 origin=5,6,7 year=2026 hours=1");
	// Class references: found by name (ClassOf::get), returned, passed.
	EXPECT(ToUtf8(Ferreal::HelloRust::Classes(Actor)) == "class=Actor actor=true back=true found=true");
	// An object of the wrong class is refused, not called.
	EXPECT(ToUtf8(Ferreal::HelloRust::Poke(reinterpret_cast<AActor*>(Component), Component)).find("own=false") != std::string::npos
		|| Logged("which is not a valid Actor"));
	delete Actor;
	delete Component;

	// Destruction frees the Rust state; a later call starts a new one.
	G->BeginDestroy();
	EXPECT(G->ChangeCount() == 0);
	G->BeginDestroy();
	delete G;
	delete T1;
	delete T2;

	if (Failures == 0)
	{
		std::printf("mock-ue: all checks passed\n");
	}
	return Failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}
