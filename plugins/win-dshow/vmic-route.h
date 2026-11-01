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
	if (saved && *saved) {
		for (size_t i = 0; i < count; i++) {
			if (strcmp(ids[i], saved) == 0) {
				return ids[i];
			}
		}
	}
	for (size_t c = 0; c < sizeof(vmic_route_cables) / sizeof(vmic_route_cables[0]); c++) {
		for (size_t i = 0; i < count; i++) {
			if (strncmp(names[i], vmic_route_cables[c], strlen(vmic_route_cables[c])) == 0) {
				return ids[i];
			}
		}
	}
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
	if (n > r->cap - r->len) {
		return false;
	}
	for (size_t i = 0; i < n; i++) {
		r->buf[(r->head + r->len + i) % r->cap] = data[i];
	}
	r->len += n;
	return true;
}

static inline size_t vmic_ring_pop(struct vmic_ring *r, float *out, size_t max)
{
	size_t n = max < r->len ? max : r->len;
	for (size_t i = 0; i < n; i++) {
		out[i] = r->buf[(r->head + i) % r->cap];
	}
	r->head = (r->head + n) % r->cap;
	r->len -= n;
	return n;
}
