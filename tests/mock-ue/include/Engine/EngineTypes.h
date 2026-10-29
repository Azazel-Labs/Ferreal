#pragma once
#include "CoreMinimal.h"

/** As the engine's: an old namespaced enum, which reflection holds in a byte. */
namespace ECollisionEnabled
{
	enum Type : int
	{
		NoCollision,
		QueryOnly,
		PhysicsOnly,
		QueryAndPhysics,
		ProbeOnly,
		QueryAndProbe,
	};
}
