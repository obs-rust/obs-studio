#include "libobs/build/libobs.pch.h"

struct obs_encoder_priming_samples_fields obs_encoder_priming_samples_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_priming_samples_fields){encoder->info.get_priming_samples, encoder->context.data};
}
