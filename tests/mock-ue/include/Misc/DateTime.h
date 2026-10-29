#pragma once
#include "CoreMinimal.h"

/** As the engine's: ticks of 100 nanoseconds, kept private. */
struct FDateTime
{
	FDateTime() = default;
	explicit FDateTime(int64 InTicks) : Ticks(InTicks) {}
	int64 GetTicks() const { return Ticks; }
private:
	int64 Ticks = 0;
};
