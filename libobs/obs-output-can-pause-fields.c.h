#include "libobs/build/libobs.pch.h"

/* Private borrowed field seam; the caller supplies a live output. */
const uint32_t *obs_output_can_pause_flags(const obs_output_t *output);
const uint32_t *obs_output_can_pause_flags(const obs_output_t *output)
{
	return &output->info.flags;
}
