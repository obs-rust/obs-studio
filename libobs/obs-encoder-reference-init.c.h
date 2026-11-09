#include "libobs/build/libobs.pch.h"

/* The reference calls the unchanged body; production supplies this symbol in Rust. */
bool obs_encoder_init_steps(void *encoder, const char *name, void *settings, void *hotkeys,
			    const struct obs_encoder_init_fields *fields)
{
	(void)fields;
	return init_encoder(encoder, name, settings, hotkeys);
}
