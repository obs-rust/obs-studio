#include "libobs/build/libobs.pch.h"

/* Defined later by obs-output.c.h in the core-output unity group. */
static inline bool active(const struct obs_output *output);
static inline bool reconnecting(const struct obs_output *output);
static inline bool delay_active(const struct obs_output *output);

/* Whether the output is using its encoders or about to: it is starting,
 * running, reconnecting or delaying. */
bool obs_output_needs_encoders(const struct obs_output *output)
{
	return active(output) || os_atomic_load_bool(&output->starting) || reconnecting(output) || delay_active(output);
}

/* Whether the output is about to use its encoders: it is starting (a stream
 * still connecting), reconnecting or delaying. */
bool obs_output_starting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->starting) || reconnecting(output) || delay_active(output);
}
