// Drives the example's UGreeter (Rust) through the generated C++ glue, in the mock engine.

#include "Ferreal/Greeter.h"
#include "Ferreal/FerrealHelloRust.h"
#include "Components/PrimitiveComponent.h"
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

		const int32 Str = sizeof(FString), StrAlign = alignof(FString);
		const int32 Ptr = sizeof(UObject*);
		AddFunction(Actor, "SetActorHiddenInGame", {Param("bNewHidden", "BoolProperty", 1, 1)},
			[](UFunction& F, UObject*, uint8* P) { State.bHidden = F.Param<bool>(P, 0); });
		AddFunction(Actor, "K2_SetActorLocation",
			{StructParam<FVector>("NewLocation", &Vector), Param("bSweep", "BoolProperty", 1, 1), StructParam<FVector>("SweepHitResult", &HitResult), Param("bTeleport", "BoolProperty", 1, 1), Return("BoolProperty", 1, 1)},
			[](UFunction&, UObject*, uint8*) {});
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
		// ECollisionEnabled::Type is an `enum Type : int` passed as itself: an EnumProperty of 4 bytes.
		AddFunction(PrimitiveComponent, "SetCollisionEnabled", {Param("NewType", "EnumProperty", 4, 4)},
			[](UFunction& F, UObject*, uint8* P) { State.Collision = F.Param<int32>(P, 0); });
		AddFunction(PrimitiveComponent, "GetCollisionEnabled", {Return("EnumProperty", 4, 4)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<int32>(P, 0) = State.Collision; });
		AddFunction(Strings, "Concat_StrStr", {Param("A", "StrProperty", Str, StrAlign), Param("B", "StrProperty", Str, StrAlign), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject* Self, uint8* P) { F.Param<FString>(P, 2) = FString(F.Param<FString>(P, 0).Str + F.Param<FString>(P, 1).Str); (void)Self; });
		AddFunction(Strings, "Conv_IntToString", {Param("InInt", "IntProperty", 4, 4), Return("StrProperty", Str, StrAlign)},
			[](UFunction& F, UObject*, uint8* P) { F.Param<FString>(P, 1) = FString::Printf(u"%d", F.Param<int32>(P, 0)); });
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
	const std::string Report = ToUtf8(Ferreal::HelloRust::Poke(Actor, Component));
	EXPECT(Report == "Rusty actor own=true turned=true z=3 tagged=true collision=3 age=0");
	EXPECT(State.bHidden && State.Owner == Actor && State.Rotation.Yaw == 20.0 && State.Rotation.Roll == 30.0);
	EXPECT(!State.bVisible && State.bPropagate && State.Collision == 3);
	EXPECT(Logged("/Script/Engine.Actor.GetGameTimeSinceCreation cannot be called from Rust"));
	// C++ written inline in the Rust (cpp!): a plain method, FMath, a string passed both ways.
	EXPECT(ToUtf8(Ferreal::HelloRust::Label(Actor, 1)) == "Rusty actor x2");
	EXPECT(ToUtf8(Ferreal::HelloRust::Label(Actor, 5)) == "Rusty actor x5");
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
