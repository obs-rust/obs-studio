#include "libobs/build/libobs.pch.h"

struct obs_encoder_roi_state_fields obs_encoder_roi_state_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_roi_state_fields){encoder ? &encoder->roi.num : NULL,
						     encoder ? &encoder->roi_increment : NULL};
}
