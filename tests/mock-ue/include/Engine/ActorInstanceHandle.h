#pragma once
#include "CoreMinimal.h"

class AActor;

/** As the engine's, in part: an actor, or an instance that can become one (FetchActor). */
struct FActorInstanceHandle
{
	FActorInstanceHandle() = default;
	explicit FActorInstanceHandle(AActor* InActor) : Actor(InActor) {}
	AActor* FetchActor() const { return Actor; }
private:
	AActor* Actor = nullptr;
};
