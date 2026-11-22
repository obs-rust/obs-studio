#include "libobs/build/libobs.pch.h"

bool obs_reset_audio(const struct obs_audio_info *oai)
{
	struct obs_audio_info2 oai2 = {
		.samples_per_sec = oai->samples_per_sec,
		.speakers = oai->speakers,
	};

	return obs_reset_audio2(&oai2);
}
