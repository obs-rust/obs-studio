/* Compiled into each real shared library only by the cmocka registration.
 * The entry point under test stays in that library, over its own hidden obs. */
#include "module_post_load_fixture.h"

struct obs_core *obs_test_module_post_load_swap_core(struct obs_core *replacement)
{
	struct obs_core *previous = obs;
	obs = replacement;
	return previous;
}
