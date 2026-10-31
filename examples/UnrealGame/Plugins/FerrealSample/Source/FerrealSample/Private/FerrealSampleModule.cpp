#include "Modules/ModuleManager.h"
#include "Ferreal/FerrealFerrealSample.h"

class FFerrealSampleModule : public IModuleInterface
{
public:
	virtual void StartupModule() override
	{
		Ferreal::FerrealSample::Register();
	}
};

IMPLEMENT_MODULE(FFerrealSampleModule, FerrealSample)
