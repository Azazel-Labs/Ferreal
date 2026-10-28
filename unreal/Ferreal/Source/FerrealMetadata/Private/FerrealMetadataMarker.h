#pragma once

#include "CoreMinimal.h"

#include "FerrealMetadataMarker.generated.h"

// Unreal Header Tool lists a module in its manifest, and runs exporters for it, only when the
// module declares reflected types: this enum is what makes the Ferreal exporter run.
UENUM()
enum class EFerrealMetadataMarker : uint8
{
	None
};
