#include "libobs/build/libobs.pch.h"

void obs_source_remove(obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_remove"))
		return;

	if (!os_atomic_load_bool(&source->removed)) {
		obs_source_t *s = obs_source_get_ref(source);
		if (s) {
			os_atomic_set_bool(&s->removed, true);
			obs_source_dosignal(s, "source_remove", "remove");
			/* Remove from canvas if there is one. */
			if (source->canvas && !source->context.private)
				obs_canvas_remove_source(s);

			obs_source_release(s);
		}
	}
}
