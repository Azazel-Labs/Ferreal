#include "Modules/ModuleManager.h"
#include "Ferreal/FerrealHelloRust.h"

class FHelloRustModule : public IModuleInterface
{
public:
	virtual void StartupModule() override
	{
		// Optional: the generated classes register on first use; this makes exported
		// functions callable from anywhere at once.
		Ferreal::HelloRust::Register();
	}
};

IMPLEMENT_MODULE(FHelloRustModule, HelloRust)
