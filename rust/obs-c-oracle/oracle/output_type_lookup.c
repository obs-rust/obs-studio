/* One original lookup shared by output metadata and properties fixtures. */
#include "obs-internal.h"
#include <string.h>
#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *output_type_core;
#else
static _Thread_local struct obs_core *output_type_core;
#endif
#define obs output_type_core
#include "output_type_find.inc"
#undef obs

struct obs_core *oracle_output_type_exchange_core(struct obs_core *core);
struct obs_core *oracle_output_type_exchange_core(struct obs_core *core)
{
	struct obs_core *previous = output_type_core;
	output_type_core = core;
	return previous;
}
