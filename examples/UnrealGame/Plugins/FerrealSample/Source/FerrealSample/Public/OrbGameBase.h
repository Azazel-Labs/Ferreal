// The C++ parent of UOrbGame, the game's rules, which the Rust crate (../../Rust) declares.
// A world subsystem: the engine makes one in every game world (and play-in-editor's) by
// itself, so the game needs nothing placed in its level or set in a game mode.

#pragma once

#include "CoreMinimal.h"
#include "Subsystems/WorldSubsystem.h"
#include "OrbGameBase.generated.h"

UCLASS(Abstract)
class FERREALSAMPLE_API UOrbGameBase : public UTickableWorldSubsystem
{
	GENERATED_BODY()

public:
	/** For the generated subclass's constructor (subsystems have only default ones). */
	UOrbGameBase(const FObjectInitializer& ObjectInitializer) {}

	/** Overridden in Rust (OrbGame::start_game): the world has begun play. */
	virtual void StartGame() {}

	/** Overridden in Rust (OrbGame::tick_game): every frame of play. */
	virtual void TickGame(float Delta) {}

	virtual void OnWorldBeginPlay(UWorld& InWorld) override;
	virtual void Tick(float DeltaTime) override;
	virtual TStatId GetStatId() const override;

protected:
	virtual bool DoesSupportWorldType(const EWorldType::Type WorldType) const override;

private:
	bool bStarted = false;
};
