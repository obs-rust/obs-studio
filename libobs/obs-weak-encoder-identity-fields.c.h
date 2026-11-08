#include "libobs/build/libobs.pch.h"

struct obs_weak_encoder_identity_fields obs_weak_encoder_identity_fields(const obs_weak_encoder_t *weak)
{
	return (struct obs_weak_encoder_identity_fields){&weak->encoder};
}
