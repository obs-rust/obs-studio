/* Complete original unit and real adapters, with atomic read recording only. */
#include "obs-internal.h"
#include <stdlib.h>
#include <string.h>

struct starting_trace {
	const volatile bool *addresses[4];
	unsigned count;
	unsigned flip;
	struct obs_output *output;
};
static _Thread_local struct starting_trace *starting_current;
static bool starting_load(const volatile bool *flag)
{
	struct starting_trace *trace = starting_current;
	if (trace->count < 4)
		trace->addresses[trace->count] = flag;
	trace->count++;
	bool result = os_atomic_load_bool(flag);
	/* Model another writer changing a later flag after the first read. */
	if (trace->count == 1 && trace->flip)
		os_atomic_set_bool(&trace->output->starting, true);
	return result;
}
#undef os_atomic_load_bool
#define os_atomic_load_bool starting_load
static inline bool active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->active);
}
static inline bool reconnecting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->reconnecting);
}
static inline bool delay_active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->delay_active);
}
#define obs_output_needs_encoders oracle_obs_output_needs_encoders
#define obs_output_starting oracle_obs_output_starting
bool oracle_obs_output_needs_encoders(const struct obs_output *output);
bool oracle_obs_output_starting(const struct obs_output *output);
#include "output-starting-queries.inc"
#undef obs_output_starting
#undef obs_output_needs_encoders
#include "output_starting_queries_fields.inc"
#undef os_atomic_load_bool

typedef bool (*starting_query)(const void *);
bool oracle_output_starting_check(starting_query candidate, unsigned mask, bool about_to_start, bool flip);
bool oracle_output_starting_check(starting_query candidate, unsigned mask, bool about_to_start, bool flip)
{
	struct obs_output *output = calloc(1, sizeof(*output));
	struct obs_output *before = malloc(sizeof(*before));
	struct obs_output *after = malloc(sizeof(*after));
	if (!output || !before || !after) {
		free(output);
		free(before);
		free(after);
		return false;
	}
	output->active = (mask & 1) != 0;
	output->starting = (mask & 2) != 0;
	output->reconnecting = (mask & 4) != 0;
	output->delay_active = (mask & 8) != 0;
	output->data_active = output->delay_capturing = true;
	memcpy(before, output, sizeof(*output));
	struct starting_trace c = {.flip = flip, .output = output};
	starting_current = &c;
	bool expected = about_to_start ? oracle_obs_output_starting(output) : oracle_obs_output_needs_encoders(output);
	memcpy(after, output, sizeof(*output));
	memcpy(output, before, sizeof(*output));
	struct starting_trace rust = {.flip = flip, .output = output};
	starting_current = &rust;
	bool actual = candidate(output);
	starting_current = NULL;
	bool equal = expected == actual && c.count == rust.count && c.count <= 4 &&
		     memcmp(c.addresses, rust.addresses, c.count * sizeof(c.addresses[0])) == 0 &&
		     memcmp(output, after, sizeof(*output)) == 0;
	if (!flip)
		equal = equal && memcmp(output, before, sizeof(*output)) == 0;
	free(after);
	free(before);
	free(output);
	return equal;
}
