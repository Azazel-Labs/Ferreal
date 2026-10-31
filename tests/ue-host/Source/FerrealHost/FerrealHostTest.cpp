// Runs the example's Rust inside the real engine: a UCLASS written in Rust, engine functions
// called through the generated bindings (each checked against the engine's reflection
// first), and C++ written inline. `cargo xtask ue --test` runs it in a headless editor.
// Ferreal logs an error for a call it refuses, which fails the test.

#include "Misc/AutomationTest.h"
#include "Engine/Engine.h"
#include "Engine/StaticMeshActor.h"
#include "Engine/World.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Ferreal/FerrealHelloRust.h"
#include "Ferreal/Greeter.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FFerrealHelloTest, "Ferreal.Hello", EAutomationTestFlags::EditorContext | EAutomationTestFlags::ProductFilter)

bool FFerrealHelloTest::RunTest(const FString& Parameters)
{
	// A class written in Rust: its defaults and #[init], a function, an exported function
	// calling its C++ parent.
	UGreeter* Greeter = NewObject<UGreeter>(GetTransientPackage());
	TestEqual(TEXT("Greet"), Greeter->Greet(TEXT("!")), FString(TEXT("HELLO, WORLD! #101")));
	TestEqual(TEXT("Shout"), Ferreal::HelloRust::Shout(TEXT("hey"), Greeter), FString(TEXT("HEY")));
	TestEqual(TEXT("Shout reached the parent"), Greeter->LastMessage, FString(TEXT("HEY")));

	UWorld* World = UWorld::CreateWorld(EWorldType::Game, false);
	FWorldContext& Context = GEngine->CreateNewWorldContext(EWorldType::Game);
	Context.SetCurrentWorld(World);
	World->InitializeActorsForPlay(FURL());
	World->BeginPlay();

	AStaticMeshActor* Actor = World->SpawnActor<AStaticMeshActor>();
	UStaticMeshComponent* Mesh = Actor->GetStaticMeshComponent();
	Mesh->SetMobility(EComponentMobility::Movable);

	// Engine functions: an actor's, a component's, static ones; values both ways.
	const FString Poke = Ferreal::HelloRust::Poke(Actor, Mesh);
	AddInfo(TEXT("Poke: ") + Poke);
	TestTrue(TEXT("Poke"), Poke.Contains(TEXT(" own=true turned=true z=0 tagged=false collision=3")));
	TestTrue(TEXT("Poke hid the actor"), Actor->IsHidden());
	TestEqual(TEXT("Poke turned the actor"), Actor->GetActorRotation().Yaw, 20.0, 1e-3);

	// C++ inline (cpp!).
	const FString Label = Ferreal::HelloRust::Label(Actor, 5);
	AddInfo(TEXT("Label: ") + Label);
	TestEqual(TEXT("Label"), Label, Actor->GetActorNameOrLabel() + TEXT(" x5"));

	// Engine structs: FHitResult made by the engine, FTransform, both through cpp! too.
	const FString Hit = Ferreal::HelloRust::Hit(Actor, Mesh);
	AddInfo(TEXT("Hit: ") + Hit);
	TestTrue(TEXT("Hit"), Hit.StartsWith(TEXT("blocking=true time=0.5 z=3 component=true actor=true bone=Spine scale=1 ")));
	TestTrue(TEXT("Hit through cpp!"), Hit.EndsWith(TEXT(" later=1.5 Spine moved=10 next=2")));

	// Outputs: an FHitResult the engine fills (no hit: its defaults), an FVector changed.
	const FString Sweep = Ferreal::HelloRust::Sweep(Actor);
	AddInfo(TEXT("Sweep: ") + Sweep);
	TestEqual(TEXT("Sweep"), Sweep, FString(TEXT("moved=true swept=1 actor=false x=0.600 shared=false")));
	TestEqual(TEXT("Sweep moved the actor"), Actor->GetActorLocation().Z, 7.0, 1e-6);

	// Core types (NoExport), mapped to Ferreal's own.
	const FString Core = Ferreal::HelloRust::CoreTypes();
	AddInfo(TEXT("CoreTypes: ") + Core);
	TestEqual(TEXT("CoreTypes"), Core, FString(TEXT("point=4,6 cube=7 flat=3 color=255,0,0,255 box=true,3 origin=5,6,7 year=2026 hours=1")));

	// Class references: the actor's class, its parent's (found by name), the world's actors.
	const FString Classes = Ferreal::HelloRust::Classes(Actor);
	AddInfo(TEXT("Classes: ") + Classes);
	TestEqual(TEXT("Classes"), Classes, FString(TEXT("class=StaticMeshActor actor=true back=false found=true")));

	// Arrays: strings returned and passed, objects passed. The trace, down from the origin,
	// hits the top of a cube under it (at z -10, Poke's turn undone), unless the actor is ignored.
	Mesh->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cube.Cube")));
	Actor->SetActorLocation(FVector(0, 0, -60));
	Actor->SetActorRotation(FRotator::ZeroRotator);
	const FString Trace = Ferreal::HelloRust::Trace(Actor);
	AddInfo(TEXT("Trace: ") + Trace);
	TestTrue(TEXT("Trace: arrays of strings"), Trace.StartsWith(TEXT("parts=[\"a\", \"b\", \"c\"] joined=x+y ")));
	TestTrue(TEXT("Trace: the hit, its actor, none when ignored"), Trace.StartsWith(TEXT("parts=[\"a\", \"b\", \"c\"] joined=x+y hit=true z=-10")) && Trace.EndsWith(TEXT(" actor=true ignored=false")));

	GEngine->DestroyWorldContext(World);
	World->DestroyWorld(false);
	return true;
}
