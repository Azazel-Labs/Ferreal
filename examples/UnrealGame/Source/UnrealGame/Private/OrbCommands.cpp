// The game's console commands: C++ calling the FerrealSample plugin's classes, which are written in
// Rust, as it would a C++ plugin's.

#include "Engine/World.h"
#include "HAL/IConsoleManager.h"
#include "Ferreal/OrbGame.h"

DEFINE_LOG_CATEGORY_STATIC(LogUnrealGame, Log, All);

static UOrbGame* OrbGame(UWorld* World)
{
	return World ? World->GetSubsystem<UOrbGame>() : nullptr;
}

static FAutoConsoleCommandWithWorld GOrbsRound(
	TEXT("Orbs.Round"),
	TEXT("Logs the round being played and the orbs left in it."),
	FConsoleCommandWithWorldDelegate::CreateLambda([](UWorld* World)
	{
		if (UOrbGame* Game = OrbGame(World))
		{
			UE_LOG(LogUnrealGame, Display, TEXT("Orbs: round %d, %d left"), Game->Round(), Game->OrbsLeft());
		}
	}));

static FAutoConsoleCommandWithWorld GOrbsCollect(
	TEXT("Orbs.Collect"),
	TEXT("Collects every orb left in the round."),
	FConsoleCommandWithWorldDelegate::CreateLambda([](UWorld* World)
	{
		if (UOrbGame* Game = OrbGame(World))
		{
			Game->CollectAll();
		}
	}));
