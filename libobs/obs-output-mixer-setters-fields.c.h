#include "libobs/build/libobs.pch.h"

struct obs_output_mixer_setters_fields {
	size_t *mask;
	volatile bool *active;
	const char *name;
	uint32_t flags;
};
struct obs_output_mixer_setters_fields obs_output_mixer_setters_fields(obs_output_t *output);
struct obs_output_mixer_setters_fields obs_output_mixer_setters_fields(obs_output_t *output)
{
	return (struct obs_output_mixer_setters_fields){&output->mixer_mask, &output->active, output->context.name,
							output->info.flags};
}
