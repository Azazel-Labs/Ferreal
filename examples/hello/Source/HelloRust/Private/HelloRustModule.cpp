#include "Modules/ModuleManager.h"
#include "Rivet/RivetHelloRust.h"

class FHelloRustModule : public IModuleInterface
{
public:
	virtual void StartupModule() override
	{
		// Optional: the generated classes register on first use; this makes exported
		// functions callable from anywhere at once.
		Rivet::HelloRust::Register();
	}
};

IMPLEMENT_MODULE(FHelloRustModule, HelloRust)
