#include "libobs/build/libobs.pch.h"

obs_source_t *obs_get_output_source(uint32_t channel)
{
	return obs_canvas_get_channel(obs->data.main_canvas, channel);
}
