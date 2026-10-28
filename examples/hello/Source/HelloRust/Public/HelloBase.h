// The hand-written C++ parent of UGreeter, the class the Rust crate (../../rust) declares.
// Its virtual functions are what the Rust overrides (#[uoverride]); its methods are what
// the Rust calls (the #[cpp_base] trait Notify).

#pragma once

#include "CoreMinimal.h"
#include "UObject/Object.h"
#include "HelloBase.generated.h"

UCLASS(Abstract, BlueprintType)
class HELLORUST_API UHelloBase : public UObject
{
	GENERATED_BODY()

public:
	UHelloBase(const FObjectInitializer& ObjectInitializer) : Super(ObjectInitializer) {}

	/** Overridden in Rust (Greeter::on_tick), which runs after this. */
	virtual void OnTick(float Delta) { Ticks++; }

	/** Overridden in Rust (Greeter::describe). */
	virtual FString Describe() const { return TEXT("a base"); }

	/** Called from Rust (Notify::notify): keeps the message, returns how many values came. */
	int32 Notify(const FString& Message, TConstArrayView<float> Values);

	/** Called from Rust (Notify::bytes). */
	TArray<uint8> Bytes() const { return TArray<uint8>{1, 2, 250}; }

	/** Called from Rust (Notify::label). */
	FString Label() const { return TEXT("the base"); }

	UPROPERTY(VisibleAnywhere, Category = "Hello")
	int32 Ticks = 0;

	FString LastMessage;
	float LastSum = 0.0f;
};
