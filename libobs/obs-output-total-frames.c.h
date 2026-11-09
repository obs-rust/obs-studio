#include "libobs/build/libobs-ported-service-getters.pch.h"

int obs_output_get_total_frames(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_get_total_frames") ? (int)os_atomic_load_long(&output->total_frames)
								       : 0;
}
