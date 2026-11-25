#include "libobs/build/libobs.pch.h"

/* Live adapter for obs_source_update() in rust/obs-source. Called only after
 * Rust checks the source; each function reads what the C read at that point. */
obs_data_t *obs_source_update_settings(const obs_source_t *source);
obs_data_t *obs_source_update_settings(const obs_source_t *source)
{
	return source->context.settings;
}

uint32_t obs_source_update_output_flags(const obs_source_t *source);
uint32_t obs_source_update_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

volatile long *obs_source_update_defer_count(obs_source_t *source);
volatile long *obs_source_update_defer_count(obs_source_t *source)
{
	return &source->defer_update_count;
}

bool obs_source_update_can_update(const obs_source_t *source);
bool obs_source_update_can_update(const obs_source_t *source)
{
	return source->context.data && source->info.update;
}

void obs_source_update_call(obs_source_t *source);
void obs_source_update_call(obs_source_t *source)
{
	source->info.update(source->context.data, source->context.settings);
}

void obs_source_update_signal(obs_source_t *source);
void obs_source_update_signal(obs_source_t *source)
{
	obs_source_dosignal(source, "source_update", "update");
}
