#include "libobs/build/libobs.pch.h"

/* A private seam lets synthetic interleaver fixtures supply their own names
 * without replacing the public encoder getter or its native-layout projection. */
const char *obs_output_interleave_encoder_name(const obs_encoder_t *encoder);

const char *obs_output_interleave_encoder_name(const obs_encoder_t *encoder)
{
	return obs_encoder_get_name(encoder);
}
