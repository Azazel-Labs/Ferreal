// The C++ parent of AOrb, the actor the Rust crate (../../Rust) declares: what Rust cannot set
// up itself, the components and ticking, which an actor's constructor does.

#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OrbBase.generated.h"

class UStaticMeshComponent;

UCLASS(Abstract)
class FERREALSAMPLE_API AOrbBase : public AActor
{
	GENERATED_BODY()

public:
	AOrbBase(const FObjectInitializer& ObjectInitializer);

	/** A sphere, with the engine's basic material (whose "Color" Rust sets). */
	UPROPERTY(VisibleAnywhere, BlueprintReadOnly, Category = "Orb")
	TObjectPtr<UStaticMeshComponent> Mesh;
};
