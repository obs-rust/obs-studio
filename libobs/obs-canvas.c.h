/******************************************************************************
    Copyright (C) 2025 by Dennis Sädtler <saedtler@twitch.tv>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

/* The primary canvas has static name/uuid. */
static const char *MAIN_CANVAS_NAME = "Main";
static const char *MAIN_CANVAS_UUID = "6c69626f-6273-4c00-9d88-c5136d61696e";

/* obs-canvas-rules.h copies the flags and limits it decides on. */
_Static_assert(OBS_CANVAS_RULES_MAIN == MAIN, "MAIN");
_Static_assert(OBS_CANVAS_RULES_ACTIVATE == ACTIVATE, "ACTIVATE");
_Static_assert(OBS_CANVAS_RULES_MIX_AUDIO == MIX_AUDIO, "MIX_AUDIO");
_Static_assert(OBS_CANVAS_RULES_SCENE_REF == SCENE_REF, "SCENE_REF");
_Static_assert(OBS_CANVAS_RULES_EPHEMERAL == EPHEMERAL, "EPHEMERAL");
_Static_assert((OBS_CANVAS_RULES_MAIN | OBS_CANVAS_RULES_ACTIVATE | OBS_CANVAS_RULES_MIX_AUDIO |
		OBS_CANVAS_RULES_SCENE_REF) == (MAIN | PROGRAM),
	       "MAIN | PROGRAM");
_Static_assert(OBS_CANVAS_RULES_FORMAT_NONE == VIDEO_FORMAT_NONE, "VIDEO_FORMAT_NONE");
_Static_assert(OBS_CANVAS_RULES_MAX_CHANNELS == MAX_CHANNELS, "MAX_CHANNELS");

/*** Signals ***/

static const char *canvas_signals[] = {
	"void destroy(ptr canvas)",
	"void remove(ptr canvas)",
	"void video_reset(ptr canvas)",

	"void source_add(ptr canvas, ptr source)",
	"void source_remove(ptr canvas, ptr source)",
	"void source_rename(ptr source, string new_name, string prev_name)",

	"void rename(ptr source, string new_name, string prev_name)",

	"void channel_change(ptr canvas, int channel, in out ptr source, ptr prev_source)",

	NULL,
};

/*** Reference Counting ***/

/*** Creation / Destruction ***/

static obs_canvas_t *obs_canvas_create_internal(const char *name, const char *uuid, struct obs_video_info *ovi,
						uint32_t flags, bool private)
{
	struct obs_canvas *canvas = bzalloc(sizeof(struct obs_canvas));
	canvas->flags = flags;

	if (!obs_context_data_init(&canvas->context, OBS_OBJ_TYPE_CANVAS, NULL, name, uuid, NULL, private)) {
		/* obs_context_data_init() already freed the context. */
		bfree(canvas);
		return NULL;
	}

	if (!signal_handler_add_array(canvas->context.signals, canvas_signals)) {
		obs_context_data_free(&canvas->context);
		bfree(canvas);
		return NULL;
	}

	if (pthread_mutex_init_recursive(&canvas->sources_mutex) != 0) {
		obs_context_data_free(&canvas->context);
		bfree(canvas);
		return NULL;
	}

	obs_view_init(&canvas->view, obs_canvas_rules_main_view(flags) ? MAIN_VIEW : AUX_VIEW);
	obs_context_init_control(&canvas->context, canvas, (obs_destroy_cb)obs_canvas_destroy);

	/* A canvas can be created without a mix. */
	if (ovi) {
		canvas->ovi = *ovi;
		canvas->mix = obs_create_video_mix(ovi);
		if (canvas->mix) {
			canvas->mix->view = &canvas->view;
			canvas->mix->mix_audio = obs_canvas_rules_mix_audio(flags);

			pthread_mutex_lock(&obs->video.mixes_mutex);
			da_push_back(obs->video.mixes, &canvas->mix);
			pthread_mutex_unlock(&obs->video.mixes_mutex);
		}
	}

	obs_context_data_insert_uuid(&canvas->context, &obs->data.canvases_mutex, &obs->data.canvases);
	if (obs_canvas_rules_public(private)) {
		obs_context_data_insert_name(&canvas->context, &obs->data.canvases_mutex, &obs->data.named_canvases);
		canvas_dosignal(canvas, "canvas_create", NULL);
	}

	blog(LOG_DEBUG, "%scanvas '%s' (%s) created", private ? "private " : "", canvas->context.name,
	     canvas->context.uuid);

	return canvas;
}

obs_canvas_t *obs_create_main_canvas(void)
{
	const uint32_t main_flags = obs_canvas_rules_main_flags();
	return obs_canvas_create_internal(MAIN_CANVAS_NAME, MAIN_CANVAS_UUID, NULL, main_flags, false);
}

void obs_canvas_destroy(obs_canvas_t *canvas)
{
	canvas_dosignal(canvas, "canvas_destroy", "destroy");

	obs_canvas_clear_mix(canvas);

	/* Empty the list before releasing anything: HASH_CLEAR reads the
	 * table through the list head, and a release can free the head on the
	 * destruction task thread. Sources still alive are destroyed later by
	 * obs_free_data(), which cannot remove them from a freed canvas
	 * (#2038, #2049). */
	obs_source_t *source = canvas->sources;
	HASH_CLEAR(hh, *(struct obs_context_data **)&canvas->sources);

	while (source) {
		/* The release can free the source; take the next one first. */
		obs_source_t *next = source->context.hh.next;

		/* This canvas mutex is freed below; a source still in the list
		 * may have a deferred destroy that waits on it (#2547). */
		if (source->context.mutex == &canvas->sources_mutex)
			source->context.mutex = &obs->data.sources_mutex;

		/* Canvases can hold strong refs to scene sources, release them here. */
		if (obs_canvas_rules_holds_scenes(canvas->flags) && obs_source_is_scene(source))
			obs_source_release(source);

		source = next;
	}

	obs_context_data_remove_uuid(&canvas->context, &obs->data.canvases_mutex, &obs->data.canvases);
	if (obs_canvas_rules_public(canvas->context.private)) {
		obs_context_data_remove_name(&canvas->context, &obs->data.canvases_mutex, &obs->data.named_canvases);
	}

	blog(LOG_DEBUG, "%scanvas '%s' (%s) destroyed", canvas->context.private ? "private " : "", canvas->context.name,
	     canvas->context.uuid);

	pthread_mutex_destroy(&canvas->sources_mutex);
	obs_context_data_free(&canvas->context);
	obs_view_free(&canvas->view);
	bfree(canvas);
}

/*** Internal API ***/

/* Clear mixes attached to canvases */
void obs_free_canvas_mixes(void)
{
	pthread_mutex_lock(&obs->data.canvases_mutex);
	struct obs_context_data *ctx, *tmp;
	HASH_ITER (hh, (struct obs_context_data *)obs->data.canvases, ctx, tmp) {
		obs_canvas_t *canvas = (obs_canvas_t *)ctx;
		obs_canvas_clear_mix(canvas);
	}
	pthread_mutex_unlock(&obs->data.canvases_mutex);
}

bool obs_canvas_reset_video_internal(obs_canvas_t *canvas, struct obs_video_info *ovi)
{
	obs_canvas_clear_mix(canvas);

	canvas->ovi = *ovi;
	canvas->mix = obs_create_video_mix(&canvas->ovi);
	if (canvas->mix) {
		canvas->mix->view = &canvas->view;
		canvas->mix->mix_audio = obs_canvas_rules_mix_audio(canvas->flags);

		pthread_mutex_lock(&obs->video.mixes_mutex);
		da_push_back(obs->video.mixes, &canvas->mix);
		pthread_mutex_unlock(&obs->video.mixes_mutex);
	}

	canvas_dosignal(canvas, "canvas_video_reset", "video_reset");

	return !!canvas->mix;
}

void obs_canvas_insert_source(obs_canvas_t *canvas, obs_source_t *source)
{
	if (obs_canvas_rules_holds_scenes(canvas->flags) && obs_source_is_scene(source))
		obs_source_get_ref(source);
	if (source->canvas)
		obs_canvas_remove_source(source);

	source->canvas = obs_canvas_get_weak_canvas(canvas);
	obs_context_data_insert_name(&source->context, &canvas->sources_mutex, &canvas->sources);
	canvas_dosignal_source("source_add", canvas, source);
}

static bool remove_groups_items_cb(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	UNUSED_PARAMETER(scene);

	obs_source_t *source = param;
	if (item->source == source)
		obs_sceneitem_remove(item);

	return true;
}

static bool remove_groups_enum_cb(void *param, obs_source_t *scene_source)
{
	obs_source_t *source = param;
	obs_scene_t *scene = obs_scene_from_source(scene_source);

	obs_scene_enum_items(scene, remove_groups_items_cb, source);
	return true;
}

void obs_canvas_remove_source(obs_source_t *source)
{
	obs_canvas_t *canvas = obs_weak_canvas_get_canvas(source->canvas);
	/* Also when the canvas is gone: obs_free_data() releases the main
	 * canvas before it destroys the sources left alive (#2038). */
	obs_weak_canvas_release(source->canvas);
	if (canvas) {
		obs_context_data_remove_name(&source->context, &canvas->sources_mutex, &canvas->sources);

		/* The source has left the canvas name list, so its wait
		 * mutex is the global source mutex again. Keeping the canvas
		 * mutex here would dangle once the canvas is freed, and the
		 * deferred destroy reads it from the task thread (#2547). */
		if (source->context.mutex == &canvas->sources_mutex)
			source->context.mutex = &obs->data.sources_mutex;

		canvas_dosignal_source("source_remove", canvas, source);
		if (obs_canvas_rules_holds_scenes(canvas->flags) && obs_source_is_scene(source))
			obs_source_release(source);

		/* If source is a group, also remove it from all other scenes in the old canvas */
		if (obs_source_is_group(source))
			obs_canvas_enum_scenes(canvas, remove_groups_enum_cb, source);

		obs_canvas_release(canvas);
	}
	source->canvas = NULL;
}

void obs_canvas_rename_source(obs_source_t *source, const char *name)
{
	obs_canvas_t *canvas = obs_weak_canvas_get_canvas(source->canvas);
	if (canvas) {
		struct calldata data;
		char *prev_name = bstrdup(source->context.name);

		obs_context_data_setname_ht(&source->context, name, &canvas->sources);

		calldata_init(&data);
		calldata_set_ptr(&data, "source", source);
		calldata_set_string(&data, "new_name", source->context.name);
		calldata_set_string(&data, "prev_name", prev_name);

		signal_handler_signal(source->context.signals, "rename", &data);
		signal_handler_signal(canvas->context.signals, "source_rename", &data);
		if (obs_canvas_rules_global_signals(canvas->flags))
			signal_handler_signal(obs->signals, "source_rename", &data);

		calldata_free(&data);
		bfree(prev_name);

		obs_canvas_release(canvas);
	}
}
