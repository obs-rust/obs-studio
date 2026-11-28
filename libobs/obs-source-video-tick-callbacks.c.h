#include "libobs/build/libobs.pch.h"

/* The per-source callback steps of obs_source_video_tick() (obs-source.c.h,
 * their only caller): the deferred update and the show, hide, activate and
 * deactivate callbacks with their signals, moved verbatim out of
 * obs-source.c.h (#5842). No longer static: obs-internal.h declares them
 * (hidden, no EXPORT), because this file comes before obs-source.c.h in the
 * unity group. */

void obs_source_deferred_update(obs_source_t *source)
{
	if (source->context.data && source->info.update) {
		long count = os_atomic_load_long(&source->defer_update_count);
		source->info.update(source->context.data, source->context.settings);
		os_atomic_compare_swap_long(&source->defer_update_count, count, 0);
		obs_source_dosignal(source, "source_update", "update");
	}
}

void activate_source(obs_source_t *source)
{
	if (source->context.data && source->info.activate)
		source->info.activate(source->context.data);
	obs_source_dosignal(source, "source_activate", "activate");
}

void deactivate_source(obs_source_t *source)
{
	if (source->context.data && source->info.deactivate)
		source->info.deactivate(source->context.data);
	obs_source_dosignal(source, "source_deactivate", "deactivate");
}

void show_source(obs_source_t *source)
{
	if (source->context.data && source->info.show)
		source->info.show(source->context.data);
	obs_source_dosignal(source, "source_show", "show");
}

void hide_source(obs_source_t *source)
{
	if (source->context.data && source->info.hide)
		source->info.hide(source->context.data);
	obs_source_dosignal(source, "source_hide", "hide");
}
