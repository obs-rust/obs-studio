#include "libobs/build/libobs.pch.h"

void obs_output_output_caption_text1(obs_output_t *output, const char *text)
{
	if (!obs_output_valid(output, "obs_output_output_caption_text1"))
		return;
	obs_output_output_caption_text2(output, text, (double)2.0f);
}
