#include "OrbGameBase.h"

void UOrbGameBase::OnWorldBeginPlay(UWorld& InWorld)
{
	Super::OnWorldBeginPlay(InWorld);
	bStarted = true;
	StartGame();
}

void UOrbGameBase::Tick(float DeltaTime)
{
	Super::Tick(DeltaTime);
	if (bStarted)
	{
		TickGame(DeltaTime);
	}
}

TStatId UOrbGameBase::GetStatId() const
{
	RETURN_QUICK_DECLARE_CYCLE_STAT(UOrbGameBase, STATGROUP_Tickables);
}

bool UOrbGameBase::DoesSupportWorldType(const EWorldType::Type WorldType) const
{
	return WorldType == EWorldType::Game || WorldType == EWorldType::PIE;
}
