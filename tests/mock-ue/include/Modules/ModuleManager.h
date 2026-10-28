#pragma once
#include "CoreMinimal.h"
class IModuleInterface { public: virtual ~IModuleInterface() = default; virtual void StartupModule() {} };
#define IMPLEMENT_MODULE(Class, Name) static Class Name##Instance;
