#include "../utility/FirstRunWizard.hpp"

#include <cassert>

int main()
{
	// A plain first launch opens the wizard.
	assert(ShowFirstRunWizard(false, false, false, false));

	// Not the first launch, an upgrade, or outputs already running: no wizard.
	assert(!ShowFirstRunWizard(true, false, false, false));
	assert(!ShowFirstRunWizard(false, true, false, false));
	assert(!ShowFirstRunWizard(false, false, true, false));

	// #3325: --startvirtualcam and the other --start options skip it.
	assert(!ShowFirstRunWizard(false, false, false, true));
}
