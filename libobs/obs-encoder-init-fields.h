#pragma once

#include <stdbool.h>
#include <stdint.h>

/* Private C boundary. All callbacks are synchronous and non-null. */
struct obs_encoder_init_fields {
	void (*sentinel)(void *encoder, uint32_t index);
	bool (*context)(void *encoder, const char *name, void *settings, void *hotkeys);
	bool (*mutex)(void *encoder, uint32_t index, bool recursive);
	void (*defaults)(void *encoder);
	void (*defaults2)(void *encoder);
};

bool obs_encoder_init_steps(void *encoder, const char *name, void *settings, void *hotkeys,
			    const struct obs_encoder_init_fields *fields);
