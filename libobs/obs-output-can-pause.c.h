#include "libobs/build/libobs-ported-service-getters.pch.h"

bool obs_output_can_pause(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_can_pause") ? !!(output->info.flags & OBS_OUTPUT_CAN_PAUSE) : false;
}
