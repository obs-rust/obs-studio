#include "libobs/build/libobs.pch.h"

bool obs_weak_encoder_references_encoder(obs_weak_encoder_t *weak, obs_encoder_t *encoder)
{
	return weak && encoder && weak->encoder == encoder;
}
