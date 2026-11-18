/* Install synthetic records in the actual library, preserving its entry point. */
#include "core_set_locale_fixture.h"

struct obs_core *obs_test_set_locale_swap_core(struct obs_core *replacement)
{
	struct obs_core *previous = obs;
	obs = replacement;
	return previous;
}
