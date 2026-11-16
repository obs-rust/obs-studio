#include "libobs/build/libobs.pch.h"

uint32_t obs_encoder_get_priming_samples(const obs_encoder_t *encoder)
{
	if (encoder->info.get_priming_samples) {
		return encoder->info.get_priming_samples(encoder->context.data);
	}

	return 0;
}
