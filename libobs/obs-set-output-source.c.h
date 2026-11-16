#include "libobs/build/libobs.pch.h"

void obs_set_output_source(uint32_t channel, obs_source_t *source)
{
	obs_canvas_set_channel(obs->data.main_canvas, channel, source);
}
