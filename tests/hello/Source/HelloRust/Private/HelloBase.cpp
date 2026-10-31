#include "HelloBase.h"

int32 UHelloBase::Notify(const FString& Message, TConstArrayView<float> Values)
{
	LastMessage = Message;
	LastSum = 0.0f;
	for (float V : Values)
	{
		LastSum += V;
	}
	return Values.Num();
}
