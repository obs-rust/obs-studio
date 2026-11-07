/* Verbatim C baseline and production private adapter over real headers. */
#define obs_display_size oracle_obs_display_size
#include "libobs/obs-display-size.c.h"
#include "display_size_fields.inc"
#include <stddef.h>
#include <stdlib.h>

size_t oracle_display_size_layout(size_t index);
void *oracle_display_size_new(uint32_t width, uint32_t height);
void oracle_display_size_free(void *opaque);
bool oracle_display_size_unchanged(void *opaque);
typedef void (*display_size_getter_t)(obs_display_t *, uint32_t *, uint32_t *);
bool oracle_display_size_concurrent(display_size_getter_t getter);

size_t oracle_display_size_layout(size_t index)
{
	const size_t layout[] = {sizeof(struct obs_display_size_fields), _Alignof(struct obs_display_size_fields),
				 offsetof(struct obs_display_size_fields, cx),
				 offsetof(struct obs_display_size_fields, cy)};
	return index < sizeof(layout) / sizeof(layout[0]) ? layout[index] : SIZE_MAX;
}

void *oracle_display_size_new(uint32_t width, uint32_t height)
{
	struct obs_display *d = calloc(1, sizeof(*d));
	if (!d)
		return NULL;
	if (pthread_mutex_init(&d->draw_info_mutex, NULL)) {
		free(d);
		return NULL;
	}
	d->cx = width;
	d->cy = height;
	d->enabled = true;
	d->next_cx = 99;
	d->next_cy = 77;
	d->background_color = UINT32_C(0x89abcdef);
	return d;
}

void oracle_display_size_free(void *opaque)
{
	struct obs_display *d = opaque;
	pthread_mutex_destroy(&d->draw_info_mutex);
	free(d);
}

bool oracle_display_size_unchanged(void *opaque)
{
	struct obs_display *d = opaque;
	return d->enabled && d->next_cx == 99 && d->next_cy == 77 && d->background_color == UINT32_C(0x89abcdef) &&
	       !d->update_color_space && !d->swap && !d->draw_callbacks.num && !d->use_clear_workaround && !d->next &&
	       !d->prev_next;
}

static void *display_size_writer(void *opaque)
{
	struct obs_display *d = opaque;
	for (uint32_t i = 1; i <= 10000; ++i) {
		pthread_mutex_lock(&d->draw_info_mutex);
		d->cx = i;
		d->cy = ~i;
		pthread_mutex_unlock(&d->draw_info_mutex);
	}
	return NULL;
}

bool oracle_display_size_concurrent(display_size_getter_t getter)
{
	struct obs_display *d = oracle_display_size_new(0, UINT32_MAX);
	if (!d)
		return false;
	pthread_t thread;
	if (pthread_create(&thread, NULL, display_size_writer, d)) {
		oracle_display_size_free(d);
		return false;
	}
	bool valid = true;
	for (size_t i = 0; i < 10000; ++i) {
		uint32_t width, height;
		getter(d, &width, &height);
		if (height != ~width)
			valid = false;
	}
	if (pthread_join(thread, NULL))
		valid = false;
	oracle_display_size_free(d);
	return valid;
}
#undef obs_display_size
