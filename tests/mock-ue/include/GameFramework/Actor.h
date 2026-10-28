#pragma once
#include "CoreMinimal.h"
class AActor : public UObject
{
public:
	/** A plain C++ method (not reflected), as the engine's: what cpp! blocks call. */
	FString GetActorNameOrLabel() const { return FString(u"Rusty actor"); }
};
