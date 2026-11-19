#include "libobs/build/libobs.pch.h"

/* The source stays C-owned; Rust borrows only its control token. */
obs_weak_source_t *obs_source_get_weak_control(obs_source_t *source);
obs_weak_source_t *obs_source_get_weak_control(obs_source_t *source)
{
	return (obs_weak_source_t *)source->context.control;
}
