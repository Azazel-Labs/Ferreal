#pragma once
#include "CoreMinimal.h"
#include "Components/PrimitiveComponent.h"

/** As the engine's: an FVector for the network, adding no field. */
struct FVector_NetQuantize : public FVector
{
	FVector_NetQuantize() = default;
	FVector_NetQuantize(const FVector& V) : FVector(V) {}
};

/** As the engine's, in part (a bit field, a weak pointer, a field that is not public). */
struct FHitResult
{
	FHitResult() : bBlockingHit(false) {}
	uint8 bBlockingHit : 1;
	float Time = 1.0f;
	FVector_NetQuantize Location;
	TWeakObjectPtr<UPrimitiveComponent> Component;
	FName BoneName;
	float EditorOnlyTime = 0.0f;
	int32 Fixed[2] = {0, 0};
	int32 GetFaceIndex() const { return FaceIndex; }
private:
	int32 FaceIndex = -1;
};
