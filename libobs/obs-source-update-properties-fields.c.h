#include "libobs/build/libobs.pch.h"

/* The source stays C-owned; the Rust orchestrator never projects its layout. */
bool obs_source_update_properties_valid(obs_source_t *source);
bool obs_source_update_properties_valid(obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_update_properties");
}

void obs_source_update_properties_signal(obs_source_t *source);
void obs_source_update_properties_signal(obs_source_t *source)
{
	/* Source-only branch of obs_source_dosignal: no OBS core is needed. */
	struct calldata data;
	uint8_t stack[128];
	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	signal_handler_signal(source->context.signals, "update_properties", &data);
}
