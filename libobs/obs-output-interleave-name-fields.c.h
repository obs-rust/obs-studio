#include "libobs/build/libobs.pch.h"

/* Private seams let synthetic interleaver fixtures supply their own names
 * without replacing the public output and encoder getters or their
 * native-layout projections. */
const char *obs_output_interleave_encoder_name(const obs_encoder_t *encoder);
const char *obs_output_interleave_output_name(const obs_output_t *output);

const char *obs_output_interleave_encoder_name(const obs_encoder_t *encoder)
{
	return obs_encoder_get_name(encoder);
}

const char *obs_output_interleave_output_name(const obs_output_t *output)
{
	return obs_output_get_name(output);
}
