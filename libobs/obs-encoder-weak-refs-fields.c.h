#include "libobs/build/libobs.pch.h"

/* Private projections only; no reference-count or lifetime decisions. */
volatile long *obs_encoder_weak_refs_count(obs_weak_encoder_t *weak);
obs_weak_encoder_t *obs_encoder_weak_refs_control(obs_encoder_t *encoder);

volatile long *obs_encoder_weak_refs_count(obs_weak_encoder_t *weak)
{
	return &weak->ref.weak_refs;
}

obs_weak_encoder_t *obs_encoder_weak_refs_control(obs_encoder_t *encoder)
{
	return (obs_weak_encoder_t *)encoder->context.control;
}
