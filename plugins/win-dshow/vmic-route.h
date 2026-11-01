#pragma once

/* Virtual Camera audio on Windows (obs-rust/obs-studio#3183): the portable
 * part of routing a mix track to a virtual audio cable. No Windows headers,
 * so the cmocka test runs on every host. */

#include <stdbool.h>
#include <stddef.h>
#include <string.h>

/* Render endpoints that a meeting app lists as a microphone. */
static const char *const vmic_route_cables[] = {"CABLE Input", "VoiceMeeter Input"};

/* The endpoint id to render to: the saved id when it is still present,
 * otherwise the first known virtual cable, otherwise NULL (video only). */
static inline const char *vmic_route_pick(const char *saved, const char *const *names, const char *const *ids,
					  size_t count)
{
	(void)saved;
	(void)names;
	(void)ids;
	(void)count;
	return NULL;
}

/* Interleaved float samples between the audio thread and the render thread.
 * A packet that does not fit is dropped whole, so channels never shift. */
struct vmic_ring {
	float *buf;
	size_t cap;
	size_t head;
	size_t len;
};

static inline bool vmic_ring_push(struct vmic_ring *r, const float *data, size_t n)
{
	(void)r;
	(void)data;
	(void)n;
	return false;
}

static inline size_t vmic_ring_pop(struct vmic_ring *r, float *out, size_t max)
{
	(void)r;
	(void)out;
	(void)max;
	return 0;
}
