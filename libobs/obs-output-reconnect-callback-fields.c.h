#include "libobs/build/libobs.pch.h"

/* Private borrowed record; the original setter requires a non-NULL output. */
struct reconnect_callback *obs_output_reconnect_callback_fields(obs_output_t *output);
struct reconnect_callback *obs_output_reconnect_callback_fields(obs_output_t *output)
{
	return &output->reconnect_callback;
}
