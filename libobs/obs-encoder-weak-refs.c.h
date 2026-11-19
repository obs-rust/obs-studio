#include "libobs/build/libobs.pch.h"

#define get_weak(encoder) ((obs_weak_encoder_t *)encoder->context.control)

void obs_weak_encoder_addref(obs_weak_encoder_t *weak)
{
	if (!weak)
		return;

	obs_weak_ref_addref(&weak->ref);
}

void obs_weak_encoder_release(obs_weak_encoder_t *weak)
{
	if (!weak)
		return;

	if (obs_weak_ref_release(&weak->ref))
		bfree(weak);
}

obs_weak_encoder_t *obs_encoder_get_weak_encoder(obs_encoder_t *encoder)
{
	if (!encoder)
		return NULL;

	obs_weak_encoder_t *weak = get_weak(encoder);
	obs_weak_encoder_addref(weak);
	return weak;
}


#undef get_weak
