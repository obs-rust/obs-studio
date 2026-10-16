/* pipewire.c
 *
 * Copyright 2020 Georges Basile Stavracas Neto <georges.stavracas@gmail.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include "pipewire.h"

#include "formats.h"
#include "frame-layout.h"
#include "frame-pacing.h"
#include "reconnect-policy.h"

#include <util/darray.h>

#include <gio/gio.h>
#include <gio/gunixfdlist.h>
#include <glib/gstdio.h>

#include <fcntl.h>
#include <glad/glad.h>
#include <libdrm/drm_fourcc.h>
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/buffer/meta.h>
#include <spa/debug/format.h>
#include <spa/debug/types.h>
#include <spa/param/video/type-info.h>
#include <spa/utils/result.h>

//#define DEBUG_PIPEWIRE

#if !PW_CHECK_VERSION(0, 3, 62)
enum spa_meta_videotransform_value {
	SPA_META_TRANSFORMATION_None = 0,   /**< no transform */
	SPA_META_TRANSFORMATION_90,         /**< 90 degree counter-clockwise */
	SPA_META_TRANSFORMATION_180,        /**< 180 degree counter-clockwise */
	SPA_META_TRANSFORMATION_270,        /**< 270 degree counter-clockwise */
	SPA_META_TRANSFORMATION_Flipped,    /**< 180 degree flipped around the vertical axis. Equivalent
						  * to a reflexion through the vertical line splitting the
						  * bufffer in two equal sized parts */
	SPA_META_TRANSFORMATION_Flipped90,  /**< flip then rotate around 90 degree counter-clockwise */
	SPA_META_TRANSFORMATION_Flipped180, /**< flip then rotate around 180 degree counter-clockwise */
	SPA_META_TRANSFORMATION_Flipped270, /**< flip then rotate around 270 degree counter-clockwise */
};

#define SPA_META_VideoTransform 8

struct spa_meta_videotransform {
	uint32_t transform; /**< orientation transformation that was applied to the buffer */
};
#endif

#define CURSOR_META_SIZE(width, height) \
	(sizeof(struct spa_meta_cursor) + sizeof(struct spa_meta_bitmap) + width * height * 4)

struct obs_pw_version {
	int major;
	int minor;
	int micro;
};

struct format_info {
	uint32_t spa_format;
	uint32_t drm_format;
	DARRAY(uint64_t) modifiers;
};

struct _obs_pipewire {
	int pipewire_fd;

	struct pw_thread_loop *thread_loop;
	struct pw_context *context;

	struct pw_core *core;
	struct spa_hook core_listener;
	int sync_id;

	struct obs_pw_version server_version;

	struct pw_registry *registry;
	struct spa_hook registry_listener;

	obs_pipewire_disconnected_cb disconnected_cb;
	void *disconnected_data;
	bool disconnected;
	bool reconnecting;

	/* Watches for the removal of the node the last stream was connected
	 * to, for owners that set a disconnected callback. */
	struct pw_registry *node_registry;
	struct spa_hook node_registry_listener;
	uint32_t watched_node;

	GPtrArray *streams;
};

struct obs_pw_crop {
	bool valid;
	int x, y;
	uint32_t width, height;
};

/* A captured screen frame waiting to be drawn, or being drawn. The texture
 * belongs to OBS: frames in shared memory are uploaded into it, DMA-BUF
 * frames are copied into the texrender's texture as they arrive, so the
 * PipeWire buffer goes back to the compositor at once. */
struct obs_pw_frame {
	gs_texture_t *texture; /* NULL: the slot is free */
	bool owns_texture;     /* else the texture is the texrender's */
	gs_texrender_t *texrender;
	enum gs_color_format texrender_format;
	enum spa_meta_videotransform_value transform;
	struct obs_pw_crop crop;
};

/* Every frame the pacer can hold, plus the one being pushed. */
#define OBS_PW_FRAME_SLOTS (OBS_PW_FRAME_PACER_MAX_FRAMES + 1)

struct _obs_pipewire_stream {
	obs_pipewire *obs_pw;
	obs_source_t *source;

	/* The frame drawn on this video tick (one of frames[]) */
	gs_texture_t *texture;

	/* Frames advance at most one per video tick (obsproject/obs-studio#13292).
	 * Pushed on the PipeWire thread and advanced in video_render, both
	 * inside the graphics context, which serialises them. */
	struct obs_pw_frame_pacer pacer;
	struct obs_pw_frame frames[OBS_PW_FRAME_SLOTS];
	uint64_t paced_video_time;

	struct pw_stream *stream;
	struct spa_hook stream_listener;
	struct spa_source *reneg;

	struct spa_video_info format;

	enum spa_meta_videotransform_value transform;

	struct obs_pw_crop crop;

	struct {
		bool visible;
		bool valid;
		int x, y;
		int hotspot_x, hotspot_y;
		int width, height;
		gs_texture_t *texture;
	} cursor;

	struct obs_video_info video_info;
	bool negotiated;
	bool streamed;
	bool bad_frame_layout_logged;

	DARRAY(struct format_info) format_info;

	struct {
		struct spa_rectangle rect;
		bool set;
	} resolution;

	struct {
		struct spa_fraction fraction;
		bool set;
	} framerate;
};

/* auxiliary methods */

static bool parse_pw_version(struct obs_pw_version *dst, const char *version)
{
	int n_matches = sscanf(version, "%d.%d.%d", &dst->major, &dst->minor, &dst->micro);
	return n_matches == 3;
}

static bool check_pw_version(const struct obs_pw_version *pw_version, int major, int minor, int micro)
{
	if (pw_version->major != major)
		return pw_version->major > major;
	if (pw_version->minor != minor)
		return pw_version->minor > minor;
	return pw_version->micro >= micro;
}

static void update_pw_versions(obs_pipewire *obs_pw, const char *version)
{
	blog(LOG_INFO, "[pipewire] Server version: %s", version);
	blog(LOG_INFO, "[pipewire] Library version: %s", pw_get_library_version());
	blog(LOG_INFO, "[pipewire] Header version: %s", pw_get_headers_version());

	if (!parse_pw_version(&obs_pw->server_version, version))
		blog(LOG_WARNING, "[pipewire] failed to parse server version");
}

static void teardown_pipewire(obs_pipewire *obs_pw)
{
	if (obs_pw->thread_loop) {
		pw_thread_loop_wait(obs_pw->thread_loop);
		pw_thread_loop_stop(obs_pw->thread_loop);
	}

	if (obs_pw->registry) {
		pw_proxy_destroy((struct pw_proxy *)obs_pw->registry);
		obs_pw->registry = NULL;
	}

	if (obs_pw->node_registry) {
		pw_proxy_destroy((struct pw_proxy *)obs_pw->node_registry);
		obs_pw->node_registry = NULL;
	}

	g_clear_pointer(&obs_pw->context, pw_context_destroy);
	g_clear_pointer(&obs_pw->thread_loop, pw_thread_loop_destroy);

	if (obs_pw->pipewire_fd > 0) {
		close(obs_pw->pipewire_fd);
		obs_pw->pipewire_fd = 0;
	}
}

static inline bool has_effective_crop(obs_pipewire_stream *obs_pw_stream)
{
	return obs_pw_stream->crop.valid && (obs_pw_stream->crop.x != 0 || obs_pw_stream->crop.y != 0 ||
					     obs_pw_stream->crop.width < obs_pw_stream->format.info.raw.size.width ||
					     obs_pw_stream->crop.height < obs_pw_stream->format.info.raw.size.height);
}

static int get_buffer_flip(obs_pipewire_stream *obs_pw_stream)
{
	int flip = 0;

	switch (obs_pw_stream->transform) {
	case SPA_META_TRANSFORMATION_Flipped:
	case SPA_META_TRANSFORMATION_Flipped180:
		flip = GS_FLIP_U;
		break;
	case SPA_META_TRANSFORMATION_Flipped90:
	case SPA_META_TRANSFORMATION_Flipped270:
		flip = GS_FLIP_V;
		break;
	case SPA_META_TRANSFORMATION_None:
	case SPA_META_TRANSFORMATION_90:
	case SPA_META_TRANSFORMATION_180:
	case SPA_META_TRANSFORMATION_270:
		break;
	}

	return flip;
}

static bool push_rotation(obs_pipewire_stream *obs_pw_stream)
{
	double offset_x = 0;
	double offset_y = 0;
	double rotation = 0;
	bool has_crop;

	has_crop = has_effective_crop(obs_pw_stream);

	switch (obs_pw_stream->transform) {
	case SPA_META_TRANSFORMATION_Flipped:
	case SPA_META_TRANSFORMATION_None:
		rotation = 0;
		break;
	case SPA_META_TRANSFORMATION_Flipped90:
	case SPA_META_TRANSFORMATION_90:
		rotation = 90;
		offset_x = 0;
		offset_y = has_crop ? obs_pw_stream->crop.height : obs_pw_stream->format.info.raw.size.height;
		break;
	case SPA_META_TRANSFORMATION_Flipped180:
	case SPA_META_TRANSFORMATION_180:
		rotation = 180;
		offset_x = has_crop ? obs_pw_stream->crop.width : obs_pw_stream->format.info.raw.size.width;
		offset_y = has_crop ? obs_pw_stream->crop.height : obs_pw_stream->format.info.raw.size.height;
		break;
	case SPA_META_TRANSFORMATION_Flipped270:
	case SPA_META_TRANSFORMATION_270:
		rotation = 270;
		offset_x = has_crop ? obs_pw_stream->crop.width : obs_pw_stream->format.info.raw.size.width;
		offset_y = 0;
		break;
	}

	if (rotation != 0) {
		gs_matrix_push();
		gs_matrix_rotaa4f(0.0f, 0.0f, 1.0f, RAD((float)rotation));
		gs_matrix_translate3f(-offset_x, -offset_y, 0.0f);
	}

	return rotation != 0;
}

static const uint32_t supported_formats_async[] = {
	SPA_VIDEO_FORMAT_RGBA,
	SPA_VIDEO_FORMAT_YUY2,
};

#define N_SUPPORTED_FORMATS_ASYNC (sizeof(supported_formats_async) / sizeof(supported_formats_async[0]))

static const uint32_t supported_formats_sync[] = {
	SPA_VIDEO_FORMAT_BGRA,       SPA_VIDEO_FORMAT_RGBA,       SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_RGBx,
#if PW_CHECK_VERSION(0, 3, 41)
	SPA_VIDEO_FORMAT_ABGR_210LE, SPA_VIDEO_FORMAT_xBGR_210LE,
#endif
};

#define N_SUPPORTED_FORMATS_SYNC (sizeof(supported_formats_sync) / sizeof(supported_formats_sync[0]))

static void swap_texture_red_blue(gs_texture_t *texture)
{
	GLuint gl_texure = *(GLuint *)gs_texture_get_obj(texture);

	glBindTexture(GL_TEXTURE_2D, gl_texure);
	glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_SWIZZLE_B, GL_RED);
	glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_SWIZZLE_R, GL_BLUE);
	glBindTexture(GL_TEXTURE_2D, 0);
}

static inline struct spa_pod *build_format(obs_pipewire_stream *obs_pw_stream, struct spa_pod_builder *b,
					   uint32_t format, uint64_t *modifiers, size_t modifier_count)
{
	struct spa_rectangle max_resolution = SPA_RECTANGLE(8192, 4320);
	struct spa_rectangle min_resolution = SPA_RECTANGLE(1, 1);
	struct spa_rectangle resolution;
	struct spa_pod_frame format_frame;
	struct spa_fraction max_framerate;
	struct spa_fraction min_framerate;
	struct spa_fraction framerate;

	if (obs_pw_stream->framerate.set) {
		framerate = obs_pw_stream->framerate.fraction;
		min_framerate = obs_pw_stream->framerate.fraction;
		max_framerate = obs_pw_stream->framerate.fraction;
	} else {
		framerate = SPA_FRACTION(obs_pw_stream->video_info.fps_num, obs_pw_stream->video_info.fps_den);
		min_framerate = SPA_FRACTION(0, 1);
		max_framerate = SPA_FRACTION(360, 1);
	}

	if (obs_pw_stream->resolution.set) {
		resolution = obs_pw_stream->resolution.rect;
		min_resolution = obs_pw_stream->resolution.rect;
		max_resolution = obs_pw_stream->resolution.rect;
	} else {
		resolution =
			SPA_RECTANGLE(obs_pw_stream->video_info.output_width, obs_pw_stream->video_info.output_height);
		min_resolution = SPA_RECTANGLE(1, 1);
		max_resolution = SPA_RECTANGLE(8192, 4320);
	}

	/* Make an object of type SPA_TYPE_OBJECT_Format and id SPA_PARAM_EnumFormat.
	 * The object type is important because it defines the properties that are
	 * acceptable. The id gives more context about what the object is meant to
	 * contain. In this case we enumerate supported formats. */
	spa_pod_builder_push_object(b, &format_frame, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat);
	/* add media type and media subtype properties */
	spa_pod_builder_add(b, SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video), 0);
	spa_pod_builder_add(b, SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), 0);

	/* formats */
	spa_pod_builder_add(b, SPA_FORMAT_VIDEO_format, SPA_POD_Id(format), 0);

	/* modifier */
	if (modifier_count > 0) {
		struct spa_pod_frame modifier_frame;

		/* build an enumeration of modifiers */
		spa_pod_builder_prop(b, SPA_FORMAT_VIDEO_modifier,
				     SPA_POD_PROP_FLAG_MANDATORY | SPA_POD_PROP_FLAG_DONT_FIXATE);

		spa_pod_builder_push_choice(b, &modifier_frame, SPA_CHOICE_Enum, 0);

		/* The first element of choice pods is the preferred value. Here
		 * we arbitrarily pick the first modifier as the preferred one.
		 */
		spa_pod_builder_long(b, modifiers[0]);

		/* modifiers from  an array */
		for (uint32_t i = 0; i < modifier_count; i++)
			spa_pod_builder_long(b, modifiers[i]);

		spa_pod_builder_pop(b, &modifier_frame);
	}
	/* add size and framerate ranges */
	spa_pod_builder_add(b, SPA_FORMAT_VIDEO_size,
			    SPA_POD_CHOICE_RANGE_Rectangle(&resolution, &min_resolution, &max_resolution),
			    SPA_FORMAT_VIDEO_framerate,
			    SPA_POD_CHOICE_RANGE_Fraction(&framerate, &min_framerate, &max_framerate), 0);
	return spa_pod_builder_pop(b, &format_frame);
}

static bool build_format_params(obs_pipewire_stream *obs_pw_stream, struct spa_pod_builder *pod_builder,
				const struct spa_pod ***param_list, uint32_t *n_params)
{
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;
	uint32_t params_count = 0;

	const struct spa_pod **params;

	if (!obs_pw_stream->format_info.num) {
		blog(LOG_ERROR, "[pipewire] No format found while building param pointers");
		return false;
	}

	params = bzalloc(2 * obs_pw_stream->format_info.num * sizeof(struct spa_pod *));

	if (!params) {
		blog(LOG_ERROR, "[pipewire] Failed to allocate memory for param pointers");
		return false;
	}

	if (!check_pw_version(&obs_pw->server_version, 0, 3, 33))
		goto build_shm;

	for (size_t i = 0; i < obs_pw_stream->format_info.num; i++) {
		if (obs_pw_stream->format_info.array[i].modifiers.num == 0) {
			continue;
		}
		params[params_count++] = build_format(obs_pw_stream, pod_builder,
						      obs_pw_stream->format_info.array[i].spa_format,
						      obs_pw_stream->format_info.array[i].modifiers.array,
						      obs_pw_stream->format_info.array[i].modifiers.num);
	}

build_shm:
	for (size_t i = 0; i < obs_pw_stream->format_info.num; i++) {
		params[params_count++] = build_format(obs_pw_stream, pod_builder,
						      obs_pw_stream->format_info.array[i].spa_format, NULL, 0);
	}
	*param_list = params;
	*n_params = params_count;
	return true;
}

static bool drm_format_available(uint32_t drm_format, uint32_t *drm_formats, size_t n_drm_formats)
{
	for (size_t j = 0; j < n_drm_formats; j++) {
		if (drm_format == drm_formats[j]) {
			return true;
		}
	}
	return false;
}

static void init_format_info_async(obs_pipewire_stream *obs_pw_stream)
{
	da_init(obs_pw_stream->format_info);

	for (size_t i = 0; i < N_SUPPORTED_FORMATS_ASYNC; i++) {
		struct obs_pw_video_format obs_pw_video_format;
		struct format_info *info;
		if (!obs_pw_video_format_from_spa_format(supported_formats_async[i], &obs_pw_video_format))
			continue;

		info = da_push_back_new(obs_pw_stream->format_info);
		da_init(info->modifiers);
		info->spa_format = obs_pw_video_format.spa_format;
		info->drm_format = obs_pw_video_format.drm_format;
	}
}

static void init_format_info_sync(obs_pipewire_stream *obs_pw_stream)
{
	da_init(obs_pw_stream->format_info);

	obs_enter_graphics();

	enum gs_dmabuf_flags dmabuf_flags;
	uint32_t *drm_formats = NULL;
	size_t n_drm_formats = 0;

	bool capabilities_queried = gs_query_dmabuf_capabilities(&dmabuf_flags, &drm_formats, &n_drm_formats);

	for (size_t i = 0; i < N_SUPPORTED_FORMATS_SYNC; i++) {
		struct obs_pw_video_format obs_pw_video_format;
		struct format_info *info;
		if (!obs_pw_video_format_from_spa_format(supported_formats_sync[i], &obs_pw_video_format) ||
		    obs_pw_video_format.gs_format == GS_UNKNOWN)
			continue;

		info = da_push_back_new(obs_pw_stream->format_info);
		da_init(info->modifiers);
		info->spa_format = obs_pw_video_format.spa_format;
		info->drm_format = obs_pw_video_format.drm_format;

		if (!capabilities_queried ||
		    !drm_format_available(obs_pw_video_format.drm_format, drm_formats, n_drm_formats))
			continue;

		size_t n_modifiers;
		uint64_t *modifiers = NULL;
		if (gs_query_dmabuf_modifiers_for_format(obs_pw_video_format.drm_format, &modifiers, &n_modifiers)) {
			da_push_back_array(info->modifiers, modifiers, n_modifiers);
		}
		bfree(modifiers);

		if (dmabuf_flags & GS_DMABUF_FLAG_IMPLICIT_MODIFIERS_SUPPORTED) {
			uint64_t modifier_implicit = DRM_FORMAT_MOD_INVALID;
			da_push_back(info->modifiers, &modifier_implicit);
		}
	}
	obs_leave_graphics();

	bfree(drm_formats);
}

static void init_format_info(obs_pipewire_stream *obs_pw_stream, const struct obs_pw_video_format *selected_format)
{
	if (selected_format) {
		struct format_info *info;

		da_init(obs_pw_stream->format_info);

		info = da_push_back_new(obs_pw_stream->format_info);
		da_init(info->modifiers);
		info->spa_format = selected_format->spa_format;
		info->drm_format = selected_format->drm_format;
	} else {
		uint32_t output_flags;

		output_flags = obs_source_get_output_flags(obs_pw_stream->source);

		if (output_flags & OBS_SOURCE_VIDEO) {
			if (output_flags & OBS_SOURCE_ASYNC)
				init_format_info_async(obs_pw_stream);
			else
				init_format_info_sync(obs_pw_stream);
		}
	}
}

static void clear_format_info(obs_pipewire_stream *obs_pw_stream)
{
	for (size_t i = 0; i < obs_pw_stream->format_info.num; i++) {
		da_free(obs_pw_stream->format_info.array[i].modifiers);
	}
	da_free(obs_pw_stream->format_info);
}

static void remove_modifier_from_format(obs_pipewire_stream *obs_pw_stream, uint32_t spa_format, uint64_t modifier)
{
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;

	for (size_t i = 0; i < obs_pw_stream->format_info.num; i++) {
		if (obs_pw_stream->format_info.array[i].spa_format != spa_format)
			continue;

		if (!check_pw_version(&obs_pw->server_version, 0, 3, 40)) {
			da_erase_range(obs_pw_stream->format_info.array[i].modifiers, 0,
				       obs_pw_stream->format_info.array[i].modifiers.num - 1);
			continue;
		}

		int idx = da_find(obs_pw_stream->format_info.array[i].modifiers, &modifier, 0);
		while (idx != -1) {
			da_erase(obs_pw_stream->format_info.array[i].modifiers, idx);
			idx = da_find(obs_pw_stream->format_info.array[i].modifiers, &modifier, 0);
		}
	}
}

static void renegotiate_format(void *data, uint64_t expirations)
{
	UNUSED_PARAMETER(expirations);
	obs_pipewire_stream *obs_pw_stream = (obs_pipewire_stream *)data;
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;
	const struct spa_pod **params = NULL;

	blog(LOG_INFO, "[pipewire] Renegotiating stream");

	pw_thread_loop_lock(obs_pw->thread_loop);

	uint8_t params_buffer[4096];
	struct spa_pod_builder pod_builder = SPA_POD_BUILDER_INIT(params_buffer, sizeof(params_buffer));
	uint32_t n_params;
	if (!build_format_params(obs_pw_stream, &pod_builder, &params, &n_params)) {
		teardown_pipewire(obs_pw);
		pw_thread_loop_unlock(obs_pw->thread_loop);
		return;
	}

	pw_stream_update_params(obs_pw_stream->stream, params, n_params);
	pw_thread_loop_unlock(obs_pw->thread_loop);
	bfree(params);
}

/* ------------------------------------------------- */

static void return_unused_pw_buffer(struct pw_stream *stream, struct pw_buffer *b)
{
#if PW_CHECK_VERSION(1, 2, 0)
	struct spa_buffer *buffer = b->buffer;
	struct spa_data *last_data = &buffer->datas[buffer->n_datas - 1];
	struct spa_meta_sync_timeline *synctimeline =
		spa_buffer_find_meta_data(buffer, SPA_META_SyncTimeline, sizeof(struct spa_meta_sync_timeline));

	if (synctimeline && (last_data->type == SPA_DATA_SyncObj))
		gs_sync_signal_syncobj_timeline_point(last_data->fd, synctimeline->release_point);
#endif
	pw_stream_queue_buffer(stream, b);
}

static inline struct pw_buffer *find_latest_buffer(struct pw_stream *stream)
{
	struct pw_buffer *b;

	/* Find the most recent buffer */
	b = NULL;
	while (true) {
		struct pw_buffer *aux = pw_stream_dequeue_buffer(stream);
		if (!aux)
			break;
		if (b)
			return_unused_pw_buffer(stream, b);
		b = aux;
	}

	return b;
}

static uint32_t get_spa_buffer_plane_count(const struct spa_buffer *buffer)
{
	uint32_t plane_count = 0;

	while (plane_count < buffer->n_datas) {
		if (buffer->datas[plane_count].type == SPA_DATA_DmaBuf)
			plane_count++;
		else
			break;
	}

	return plane_count;
}

static enum video_colorspace video_colorspace_from_spa_color_matrix(enum spa_video_color_matrix matrix)
{
	switch (matrix) {
	case SPA_VIDEO_COLOR_MATRIX_RGB:
		return VIDEO_CS_DEFAULT;
	case SPA_VIDEO_COLOR_MATRIX_BT601:
		return VIDEO_CS_601;
	case SPA_VIDEO_COLOR_MATRIX_BT709:
		return VIDEO_CS_709;
	default:
		return VIDEO_CS_DEFAULT;
	}
}

static enum video_range_type video_color_range_from_spa_color_range(enum spa_video_color_range colorrange)
{
	switch (colorrange) {
	case SPA_VIDEO_COLOR_RANGE_0_255:
		return VIDEO_RANGE_FULL;
	case SPA_VIDEO_COLOR_RANGE_16_235:
		return VIDEO_RANGE_PARTIAL;
	default:
		return VIDEO_RANGE_DEFAULT;
	}
}

static bool prepare_obs_frame(obs_pipewire_stream *obs_pw_stream, struct obs_source_frame *frame,
			      struct obs_pw_video_format *obs_pw_video_format)
{
	frame->width = obs_pw_stream->format.info.raw.size.width;
	frame->height = obs_pw_stream->format.info.raw.size.height;

	video_format_get_parameters(video_colorspace_from_spa_color_matrix(obs_pw_stream->format.info.raw.color_matrix),
				    video_color_range_from_spa_color_range(obs_pw_stream->format.info.raw.color_range),
				    frame->color_matrix, frame->color_range_min, frame->color_range_max);

	if (!obs_pw_video_format_from_spa_format(obs_pw_stream->format.info.raw.format, obs_pw_video_format) ||
	    obs_pw_video_format->video_format == VIDEO_FORMAT_NONE)
		return false;

	frame->format = obs_pw_video_format->video_format;
	return true;
}

static void process_video_async(obs_pipewire_stream *obs_pw_stream)
{
	struct spa_buffer *buffer;
	struct pw_buffer *b;
	bool has_buffer;

	b = find_latest_buffer(obs_pw_stream->stream);
	if (!b) {
		blog(LOG_DEBUG, "[pipewire] Out of buffers!");
		return;
	}

	buffer = b->buffer;
	has_buffer = buffer->datas[0].chunk->size != 0;

	if (!has_buffer)
		goto done;

#ifdef DEBUG_PIPEWIRE
	blog(LOG_DEBUG, "[pipewire] Buffer has memory texture");
#endif

	struct obs_source_frame out = {0};
	struct obs_pw_video_format obs_pw_video_format;
	if (!prepare_obs_frame(obs_pw_stream, &out, &obs_pw_video_format)) {
		blog(LOG_ERROR, "[pipewire] Couldn't prepare frame");
		goto done;
	}

	if (!obs_pw_frame_layout(&out, buffer, &obs_pw_video_format)) {
		if (!obs_pw_stream->bad_frame_layout_logged) {
			blog(LOG_WARNING, "[pipewire] Dropping frames: a buffer does not hold a %ux%u %s frame",
			     out.width, out.height, obs_pw_video_format.pretty_name);
			obs_pw_stream->bad_frame_layout_logged = true;
		}
		goto done;
	}

#ifdef DEBUG_PIPEWIRE
	blog(LOG_DEBUG, "[pipewire] Frame info: Format: %s, Planes: %u", get_video_format_name(out.format),
	     buffer->n_datas);
	for (uint32_t i = 0; i < buffer->n_datas && i < MAX_AV_PLANES; i++) {
		blog(LOG_DEBUG, "[pipewire] Plane %u: Dataptr:%p, Linesize:%d", i, out.data[i], out.linesize[i]);
	}
#endif

	obs_source_output_video(obs_pw_stream->source, &out);

done:
	pw_stream_queue_buffer(obs_pw_stream->stream, b);
}

static struct obs_pw_frame *get_free_frame(obs_pipewire_stream *obs_pw_stream)
{
	for (size_t i = 0; i < OBS_PW_FRAME_SLOTS; i++) {
		if (!obs_pw_stream->frames[i].texture)
			return &obs_pw_stream->frames[i];
	}
	return NULL;
}

static void release_frame(obs_pipewire_stream *obs_pw_stream, uint64_t id)
{
	struct obs_pw_frame *frame;

	if (id == OBS_PW_FRAME_NONE)
		return;

	frame = &obs_pw_stream->frames[id];
	if (frame->owns_texture)
		gs_texture_destroy(frame->texture);
	frame->texture = NULL;
	frame->owns_texture = false;
}

static void queue_frame(obs_pipewire_stream *obs_pw_stream, struct obs_pw_frame *frame)
{
	uint64_t id = (uint64_t)(frame - obs_pw_stream->frames);

	release_frame(obs_pw_stream, obs_pw_frame_pacer_push(&obs_pw_stream->pacer, id));
}

/* Draws an imported DMA-BUF frame into a texture of our own, so that the
 * frame can wait for its video tick after its buffer went back to the
 * compositor. */
static gs_texture_t *copy_dmabuf_frame(struct obs_pw_frame *frame, gs_texture_t *imported, enum gs_color_format format)
{
	uint32_t width = gs_texture_get_width(imported);
	uint32_t height = gs_texture_get_height(imported);
	gs_effect_t *effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	bool previous_srgb;

	if (frame->texrender && frame->texrender_format != format)
		g_clear_pointer(&frame->texrender, gs_texrender_destroy);
	if (!frame->texrender) {
		frame->texrender = gs_texrender_create(format, GS_ZS_NONE);
		frame->texrender_format = format;
	}

	gs_texrender_reset(frame->texrender);
	if (!gs_texrender_begin(frame->texrender, width, height))
		return NULL;

	previous_srgb = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(false);
	gs_blend_state_push();
	gs_enable_blending(false);
	gs_ortho(0.0f, (float)width, 0.0f, (float)height, -100.0f, 100.0f);

	gs_effect_set_texture(gs_effect_get_param_by_name(effect, "image"), imported);
	while (gs_effect_loop(effect, "Draw"))
		gs_draw_sprite(imported, 0, width, height);

	gs_blend_state_pop();
	gs_enable_framebuffer_srgb(previous_srgb);
	gs_texrender_end(frame->texrender);

	return gs_texrender_get_texture(frame->texrender);
}

static void process_video_sync(obs_pipewire_stream *obs_pw_stream)
{
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;
	struct spa_meta_cursor *cursor;
	struct spa_meta_header *header;
	struct spa_meta_region *region;
	struct spa_meta_videotransform *video_transform;
	struct obs_pw_video_format obs_pw_video_format;
	struct obs_pw_frame *frame;
	struct spa_buffer *buffer;
	struct pw_buffer *b;
	bool has_buffer = true;
	bool release_point_exported = false;

	b = find_latest_buffer(obs_pw_stream->stream);
	if (!b) {
		blog(LOG_DEBUG, "[pipewire] Out of buffers!");
		return;
	}

	buffer = b->buffer;
	header = spa_buffer_find_meta_data(buffer, SPA_META_Header, sizeof(*header));
	if (header && (header->flags & SPA_META_HEADER_FLAG_CORRUPTED) > 0) {
		blog(LOG_ERROR, "[pipewire] buffer is corrupt");
		return_unused_pw_buffer(obs_pw_stream->stream, b);
		return;
	}

	obs_enter_graphics();

	/* The pacer holds at most OBS_PW_FRAME_PACER_MAX_FRAMES, so one is free */
	frame = get_free_frame(obs_pw_stream);
	if (!frame)
		goto read_metadata;

	// Workaround for kwin behaviour pre 5.27.5
	// Workaround for mutter behaviour pre GNOME 43
	// Only check this if !SPA_META_Header, once supported platforms update.
	has_buffer = buffer->datas[0].chunk->size != 0;
	if (!has_buffer)
		goto read_metadata;

	if (buffer->datas[0].type == SPA_DATA_DmaBuf) {
		uint32_t planes = get_spa_buffer_plane_count(buffer);
		uint32_t *offsets = alloca(sizeof(uint32_t) * planes);
		uint32_t *strides = alloca(sizeof(uint32_t) * planes);
		uint64_t *modifiers = alloca(sizeof(uint64_t) * planes);
		int *fds = alloca(sizeof(int) * planes);
		gs_texture_t *imported;
		bool use_modifiers;
		bool corrupt = false;
#if PW_CHECK_VERSION(1, 2, 0)
		struct spa_meta_sync_timeline *synctimeline =
			spa_buffer_find_meta_data(buffer, SPA_META_SyncTimeline, sizeof(struct spa_meta_sync_timeline));
#endif

#ifdef DEBUG_PIPEWIRE
		blog(LOG_DEBUG, "[pipewire] DMA-BUF info: fd:%ld, stride:%d, offset:%u, size:%dx%d",
		     buffer->datas[0].fd, buffer->datas[0].chunk->stride, buffer->datas[0].chunk->offset,
		     obs_pw_stream->format.info.raw.size.width, obs_pw_stream->format.info.raw.size.height);
#endif

		if (!obs_pw_video_format_from_spa_format(obs_pw_stream->format.info.raw.format, &obs_pw_video_format) ||
		    obs_pw_video_format.gs_format == GS_UNKNOWN) {
			blog(LOG_ERROR, "[pipewire] unsupported DMA buffer format: %d",
			     obs_pw_stream->format.info.raw.format);
			goto read_metadata;
		}

		for (uint32_t plane = 0; plane < planes; plane++) {
			fds[plane] = buffer->datas[plane].fd;
			offsets[plane] = buffer->datas[plane].chunk->offset;
			strides[plane] = buffer->datas[plane].chunk->stride;
			modifiers[plane] = obs_pw_stream->format.info.raw.modifier;
			corrupt |= (buffer->datas[plane].chunk->flags & SPA_CHUNK_FLAG_CORRUPTED) > 0;
		}

		if (corrupt) {
			blog(LOG_DEBUG, "[pipewire] buffer contains corrupted data");
			goto read_metadata;
		}

		use_modifiers = obs_pw_stream->format.info.raw.modifier != DRM_FORMAT_MOD_INVALID;
		imported = gs_texture_create_from_dmabuf(obs_pw_stream->format.info.raw.size.width,
							 obs_pw_stream->format.info.raw.size.height,
							 obs_pw_video_format.drm_format, GS_BGRX, planes, fds, strides,
							 offsets, use_modifiers ? modifiers : NULL);

		if (imported == NULL) {
			remove_modifier_from_format(obs_pw_stream, obs_pw_stream->format.info.raw.format,
						    obs_pw_stream->format.info.raw.modifier);
			pw_loop_signal_event(pw_thread_loop_get_loop(obs_pw->thread_loop), obs_pw_stream->reneg);
			goto read_metadata;
		}

		if (obs_pw_video_format.swap_red_blue)
			swap_texture_red_blue(imported);

#if PW_CHECK_VERSION(1, 2, 0)
		if (synctimeline && (buffer->n_datas == (planes + 2))) {
			assert(buffer->datas[planes].type == SPA_DATA_SyncObj);
			assert(buffer->datas[planes + 1].type == SPA_DATA_SyncObj);

			/* Copy only once the compositor has finished drawing it */
			gs_sync_t *acquire_sync = gs_sync_create_from_syncobj_timeline_point(
				buffer->datas[planes].fd, synctimeline->acquire_point);
			gs_sync_wait(acquire_sync);
			gs_sync_destroy(acquire_sync);
		} else {
			synctimeline = NULL;
		}
#endif

		frame->texture = copy_dmabuf_frame(
			frame, imported, obs_pw_video_format.gs_format == GS_R10G10B10A2 ? GS_R10G10B10A2 : GS_BGRA);
		frame->owns_texture = false;

#if PW_CHECK_VERSION(1, 2, 0)
		if (synctimeline) {
			/* The compositor may reuse the buffer once the copy is done */
			gs_sync_t *release_sync = gs_sync_create();
			gs_sync_export_syncobj_timeline_point(release_sync, buffer->datas[planes + 1].fd,
							      synctimeline->release_point);
			gs_sync_destroy(release_sync);
			release_point_exported = true;
		}
#endif
		/* Submit the copy before the buffer goes back (implicit sync) */
		gs_flush();
		gs_texture_destroy(imported);

		if (!frame->texture) {
			blog(LOG_DEBUG, "[pipewire] Couldn't copy the DMA-BUF frame");
			goto read_metadata;
		}
	} else {
		blog(LOG_DEBUG, "[pipewire] Buffer has memory texture");

		if (!obs_pw_video_format_from_spa_format(obs_pw_stream->format.info.raw.format, &obs_pw_video_format) ||
		    obs_pw_video_format.gs_format == GS_UNKNOWN) {
			blog(LOG_ERROR, "[pipewire] unsupported buffer format: %d",
			     obs_pw_stream->format.info.raw.format);
			goto read_metadata;
		}

		if ((buffer->datas[0].chunk->flags & SPA_CHUNK_FLAG_CORRUPTED) > 0) {
			blog(LOG_DEBUG, "[pipewire] buffer contains corrupted data");
			goto read_metadata;
		}

		if (buffer->datas[0].chunk->size == 0) {
			blog(LOG_DEBUG, "[pipewire] buffer contains empty data");
			goto read_metadata;
		}

		/* Where the frame is in the buffer and its row pitch, from the
		 * producer's chunk, checked against the mapped buffer. */
		struct obs_source_frame layout = {
			.width = obs_pw_stream->format.info.raw.size.width,
			.height = obs_pw_stream->format.info.raw.size.height,
			.format = obs_pw_video_format.video_format,
		};

		if (!obs_pw_frame_layout(&layout, buffer, &obs_pw_video_format)) {
			if (!obs_pw_stream->bad_frame_layout_logged) {
				blog(LOG_WARNING, "[pipewire] Dropping frames: a buffer does not hold a %ux%u %s frame",
				     layout.width, layout.height, obs_pw_video_format.pretty_name);
				obs_pw_stream->bad_frame_layout_logged = true;
			}
			goto read_metadata;
		}

		if (layout.linesize[0] == layout.width * obs_pw_video_format.bpp) {
			frame->texture = gs_texture_create(layout.width, layout.height, obs_pw_video_format.gs_format,
							   1, (const uint8_t **)(uintptr_t)&layout.data[0], GS_DYNAMIC);
		} else {
			/* Padded rows: copied one by one. */
			frame->texture = gs_texture_create(layout.width, layout.height, obs_pw_video_format.gs_format,
							   1, NULL, GS_DYNAMIC);
			if (frame->texture)
				gs_texture_set_image(frame->texture, layout.data[0], layout.linesize[0], false);
		}
		frame->owns_texture = true;

		if (!frame->texture)
			goto read_metadata;

		if (obs_pw_video_format.swap_red_blue)
			swap_texture_red_blue(frame->texture);
	}

	/* Video Crop */
	region = spa_buffer_find_meta_data(buffer, SPA_META_VideoCrop, sizeof(*region));
	if (region && spa_meta_region_is_valid(region)) {
#ifdef DEBUG_PIPEWIRE
		blog(LOG_DEBUG, "[pipewire] Crop Region available (%dx%d+%d+%d)", region->region.position.x,
		     region->region.position.y, region->region.size.width, region->region.size.height);
#endif

		frame->crop.x = region->region.position.x;
		frame->crop.y = region->region.position.y;
		frame->crop.width = region->region.size.width;
		frame->crop.height = region->region.size.height;
		frame->crop.valid = true;
	} else {
		frame->crop.valid = false;
	}

	/* Video Transform */
	video_transform = spa_buffer_find_meta_data(buffer, SPA_META_VideoTransform, sizeof(*video_transform));
	if (video_transform)
		frame->transform = video_transform->transform;
	else
		frame->transform = SPA_META_TRANSFORMATION_None;

	/* Drawn on a later video tick, see obs_pipewire_stream_video_render() */
	queue_frame(obs_pw_stream, frame);

read_metadata:

	/* Cursor */
	cursor = spa_buffer_find_meta_data(buffer, SPA_META_Cursor, sizeof(*cursor));
	obs_pw_stream->cursor.valid = cursor && spa_meta_cursor_is_valid(cursor);
	if (obs_pw_stream->cursor.visible && obs_pw_stream->cursor.valid) {
		struct spa_meta_bitmap *bitmap = NULL;

		if (cursor->bitmap_offset)
			bitmap = SPA_MEMBER(cursor, cursor->bitmap_offset, struct spa_meta_bitmap);

		if (bitmap)
			g_clear_pointer(&obs_pw_stream->cursor.texture, gs_texture_destroy);

		if (bitmap && bitmap->size.width > 0 && bitmap->size.height > 0 &&
		    obs_pw_video_format_from_spa_format(bitmap->format, &obs_pw_video_format) &&
		    obs_pw_video_format.gs_format != GS_UNKNOWN) {
			const uint8_t *bitmap_data;

			bitmap_data = SPA_MEMBER(bitmap, bitmap->offset, uint8_t);
			obs_pw_stream->cursor.hotspot_x = cursor->hotspot.x;
			obs_pw_stream->cursor.hotspot_y = cursor->hotspot.y;
			obs_pw_stream->cursor.width = bitmap->size.width;
			obs_pw_stream->cursor.height = bitmap->size.height;

			assert(obs_pw_stream->cursor.texture == NULL);
			obs_pw_stream->cursor.texture =
				gs_texture_create(obs_pw_stream->cursor.width, obs_pw_stream->cursor.height,
						  obs_pw_video_format.gs_format, 1, &bitmap_data, GS_DYNAMIC);

			if (obs_pw_video_format.swap_red_blue)
				swap_texture_red_blue(obs_pw_stream->cursor.texture);
		}

		obs_pw_stream->cursor.x = cursor->position.x;
		obs_pw_stream->cursor.y = cursor->position.y;
	}

	if (release_point_exported)
		pw_stream_queue_buffer(obs_pw_stream->stream, b);
	else
		return_unused_pw_buffer(obs_pw_stream->stream, b);

	obs_leave_graphics();
}

static void on_process_cb(void *user_data)
{
	obs_pipewire_stream *obs_pw_stream = user_data;
	uint32_t output_flags;

	output_flags = obs_source_get_output_flags(obs_pw_stream->source);

	if (output_flags & OBS_SOURCE_VIDEO) {
		if (output_flags & OBS_SOURCE_ASYNC)
			process_video_async(obs_pw_stream);
		else
			process_video_sync(obs_pw_stream);
	}
}

static void on_param_changed_cb(void *user_data, uint32_t id, const struct spa_pod *param)
{
	obs_pipewire_stream *obs_pw_stream = user_data;
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;
	struct spa_pod_builder pod_builder;
	const struct spa_pod *params[7];
	const char *format_name;
	uint32_t n_params = 0;
	uint32_t buffer_types;
	uint32_t output_flags;
	uint8_t params_buffer[1024];
	int result;
#if PW_CHECK_VERSION(1, 2, 0)
	bool supports_explicit_sync = false;
#endif

	if (!param || id != SPA_PARAM_Format)
		return;

	result = spa_format_parse(param, &obs_pw_stream->format.media_type, &obs_pw_stream->format.media_subtype);
	if (result < 0)
		return;

	if (obs_pw_stream->format.media_type != SPA_MEDIA_TYPE_video ||
	    obs_pw_stream->format.media_subtype != SPA_MEDIA_SUBTYPE_raw)
		return;

	spa_format_video_raw_parse(param, &obs_pw_stream->format.info.raw);

	output_flags = obs_source_get_output_flags(obs_pw_stream->source);

	buffer_types = 1 << SPA_DATA_MemPtr;
	bool has_modifier = spa_pod_find_prop(param, NULL, SPA_FORMAT_VIDEO_modifier) != NULL;
	if ((has_modifier || check_pw_version(&obs_pw->server_version, 0, 3, 24)) &&
	    (output_flags & OBS_SOURCE_ASYNC_VIDEO) != OBS_SOURCE_ASYNC_VIDEO) {
		buffer_types |= 1 << SPA_DATA_DmaBuf;
#if PW_CHECK_VERSION(1, 2, 0)
		obs_enter_graphics();
		supports_explicit_sync = gs_query_sync_capabilities();
		obs_leave_graphics();
#endif
	}

	blog(LOG_INFO, "[pipewire] Negotiated format:");

	format_name = spa_debug_type_find_name(spa_type_video_format, obs_pw_stream->format.info.raw.format);
	blog(LOG_INFO, "[pipewire]     Format: %d (%s)", obs_pw_stream->format.info.raw.format,
	     format_name ? format_name : "unknown format");

	if (has_modifier) {
		blog(LOG_INFO, "[pipewire]     Modifier: 0x%" PRIx64, obs_pw_stream->format.info.raw.modifier);
	}

	blog(LOG_INFO, "[pipewire]     Size: %dx%d", obs_pw_stream->format.info.raw.size.width,
	     obs_pw_stream->format.info.raw.size.height);

	blog(LOG_INFO, "[pipewire]     Framerate: %d/%d", obs_pw_stream->format.info.raw.framerate.num,
	     obs_pw_stream->format.info.raw.framerate.denom);

	/* Video crop */
	pod_builder = SPA_POD_BUILDER_INIT(params_buffer, sizeof(params_buffer));
	params[n_params++] = spa_pod_builder_add_object(&pod_builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
							SPA_PARAM_META_type, SPA_POD_Id(SPA_META_VideoCrop),
							SPA_PARAM_META_size,
							SPA_POD_Int(sizeof(struct spa_meta_region)));

	/* Cursor */
	params[n_params++] =
		spa_pod_builder_add_object(&pod_builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta, SPA_PARAM_META_type,
					   SPA_POD_Id(SPA_META_Cursor), SPA_PARAM_META_size,
					   SPA_POD_CHOICE_RANGE_Int(CURSOR_META_SIZE(64, 64), CURSOR_META_SIZE(1, 1),
								    CURSOR_META_SIZE(1024, 1024)));

	/* Buffer options */
#if PW_CHECK_VERSION(1, 2, 0)
	if (supports_explicit_sync) {
		struct spa_pod_frame dmabuf_explicit_sync_frame;

		spa_pod_builder_push_object(&pod_builder, &dmabuf_explicit_sync_frame, SPA_TYPE_OBJECT_ParamBuffers,
					    SPA_PARAM_Buffers);
		spa_pod_builder_add(&pod_builder, SPA_PARAM_BUFFERS_dataType, SPA_POD_Int(1 << SPA_DATA_DmaBuf), 0);
		spa_pod_builder_prop(&pod_builder, SPA_PARAM_BUFFERS_metaType, SPA_POD_PROP_FLAG_MANDATORY);
		spa_pod_builder_int(&pod_builder, 1 << SPA_META_SyncTimeline);

		params[n_params++] = spa_pod_builder_pop(&pod_builder, &dmabuf_explicit_sync_frame);
	}
#endif

	params[n_params++] = spa_pod_builder_add_object(&pod_builder, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
							SPA_PARAM_BUFFERS_dataType, SPA_POD_Int(buffer_types));

	/* Sync timeline */
#if PW_CHECK_VERSION(1, 2, 0)
	if (supports_explicit_sync) {
		params[n_params++] = spa_pod_builder_add_object(&pod_builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
								SPA_PARAM_META_type, SPA_POD_Id(SPA_META_SyncTimeline),
								SPA_PARAM_META_size,
								SPA_POD_Int(sizeof(struct spa_meta_sync_timeline)));
	}
#endif

	/* Meta header */
	params[n_params++] = spa_pod_builder_add_object(&pod_builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
							SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Header),
							SPA_PARAM_META_size,
							SPA_POD_Int(sizeof(struct spa_meta_header)));

#if PW_CHECK_VERSION(0, 3, 62)
	if (check_pw_version(&obs_pw->server_version, 0, 3, 62)) {
		/* Video transformation */
		params[n_params++] = spa_pod_builder_add_object(&pod_builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
								SPA_PARAM_META_type,
								SPA_POD_Id(SPA_META_VideoTransform),
								SPA_PARAM_META_size,
								SPA_POD_Int(sizeof(struct spa_meta_videotransform)));
	}
#endif
	pw_stream_update_params(obs_pw_stream->stream, params, n_params);
	obs_pw_stream->negotiated = true;
}

static void on_state_changed_cb(void *user_data, enum pw_stream_state old, enum pw_stream_state state,
				const char *error)
{
	UNUSED_PARAMETER(old);

	obs_pipewire_stream *obs_pw_stream = user_data;
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;

	blog(LOG_INFO, "[pipewire] Stream %p state: \"%s\" (error: %s)", obs_pw_stream->stream,
	     pw_stream_state_as_string(state), error ? error : "none");

	if (obs_pipewire_stream_state_is_lost(&obs_pw->disconnected, &obs_pw_stream->streamed, obs_pw->reconnecting,
					      state) &&
	    obs_pw->disconnected_cb)
		obs_pw->disconnected_cb(obs_pw->disconnected_data);
}

static const struct pw_stream_events stream_events = {
	PW_VERSION_STREAM_EVENTS,
	.state_changed = on_state_changed_cb,
	.param_changed = on_param_changed_cb,
	.process = on_process_cb,
};

static void on_core_info_cb(void *user_data, const struct pw_core_info *info)
{
	obs_pipewire *obs_pw = user_data;

	update_pw_versions(obs_pw, info->version);
}

static void on_core_error_cb(void *user_data, uint32_t id, int seq, int res, const char *message)
{
	obs_pipewire *obs_pw = user_data;

	blog(LOG_ERROR, "[pipewire] Error id:%u seq:%d res:%d (%s): %s", id, seq, res, spa_strerror(res), message);

	if (obs_pipewire_core_error_is_lost(&obs_pw->disconnected, id, res) && obs_pw->disconnected_cb)
		obs_pw->disconnected_cb(obs_pw->disconnected_data);

	pw_thread_loop_signal(obs_pw->thread_loop, FALSE);
}

static void on_node_registry_global_remove_cb(void *user_data, uint32_t id)
{
	obs_pipewire *obs_pw = user_data;

	if (obs_pipewire_node_removed_is_lost(&obs_pw->disconnected, obs_pw->watched_node, id)) {
		blog(LOG_INFO, "[pipewire] Node %u was removed", id);
		if (obs_pw->disconnected_cb)
			obs_pw->disconnected_cb(obs_pw->disconnected_data);
	}
}

static const struct pw_registry_events node_registry_events = {
	PW_VERSION_REGISTRY_EVENTS,
	.global_remove = on_node_registry_global_remove_cb,
};

static void on_core_done_cb(void *user_data, uint32_t id, int seq)
{
	obs_pipewire *obs_pw = user_data;

	if (id == PW_ID_CORE && obs_pw->sync_id == seq)
		pw_thread_loop_signal(obs_pw->thread_loop, FALSE);
}

static const struct pw_core_events core_events = {
	PW_VERSION_CORE_EVENTS,
	.info = on_core_info_cb,
	.done = on_core_done_cb,
	.error = on_core_error_cb,
};

/* obs_source_info methods */

obs_pipewire *obs_pipewire_connect_fd(int pipewire_fd)
{
	obs_pipewire *obs_pw;

	obs_pw = bzalloc(sizeof(obs_pipewire));
	obs_pw->pipewire_fd = pipewire_fd;
	obs_pw->watched_node = PW_ID_ANY;
	obs_pw->thread_loop = pw_thread_loop_new("PipeWire thread loop", NULL);
	if (!obs_pw->thread_loop) {
		blog(LOG_WARNING, "[pipewire] Error creating PipeWire thread loop: %m");
		goto fail;
	}

	/* Fails when PipeWire cannot load its client configuration
	 * (client.conf), for example when PIPEWIRE_CONFIG_DIR points at a
	 * directory without it. */
	obs_pw->context = pw_context_new(pw_thread_loop_get_loop(obs_pw->thread_loop), NULL, 0);
	if (!obs_pw->context) {
		blog(LOG_WARNING,
		     "[pipewire] Error creating PipeWire context: %m (is the PipeWire client configuration missing?)");
		goto fail;
	}

	if (pw_thread_loop_start(obs_pw->thread_loop) < 0) {
		blog(LOG_WARNING, "[pipewire] Error starting threaded mainloop");
		goto fail;
	}

	pw_thread_loop_lock(obs_pw->thread_loop);

	/* Core */
	obs_pw->core = pw_context_connect_fd(obs_pw->context, fcntl(obs_pw->pipewire_fd, F_DUPFD_CLOEXEC, 5), NULL, 0);
	if (!obs_pw->core) {
		blog(LOG_WARNING, "[pipewire] Error creating PipeWire core: %m");
		pw_thread_loop_unlock(obs_pw->thread_loop);
		goto fail;
	}

	pw_core_add_listener(obs_pw->core, &obs_pw->core_listener, &core_events, obs_pw);

	// Dispatch to receive the info core event
	obs_pw->sync_id = pw_core_sync(obs_pw->core, PW_ID_CORE, obs_pw->sync_id);
	pw_thread_loop_wait(obs_pw->thread_loop);

	pw_thread_loop_unlock(obs_pw->thread_loop);

	obs_pw->streams = g_ptr_array_new();

	return obs_pw;

fail:
	/* Stopping a loop that never started is a no-op. */
	if (obs_pw->thread_loop)
		pw_thread_loop_stop(obs_pw->thread_loop);
	g_clear_pointer(&obs_pw->context, pw_context_destroy);
	g_clear_pointer(&obs_pw->thread_loop, pw_thread_loop_destroy);
	/* The fd is ours from here on; callers do not close it on failure. */
	close(pipewire_fd);
	bfree(obs_pw);
	return NULL;
}

void obs_pipewire_add_registry_listener(obs_pipewire *obs_pw, const struct pw_registry_events *registry_events,
					void *user_data)
{
	pw_thread_loop_lock(obs_pw->thread_loop);
	obs_pw->registry = pw_core_get_registry(obs_pw->core, PW_VERSION_REGISTRY, 0);
	pw_registry_add_listener(obs_pw->registry, &obs_pw->registry_listener, registry_events, user_data);
	blog(LOG_INFO, "[pipewire] Created registry %p", obs_pw->registry);
	pw_thread_loop_unlock(obs_pw->thread_loop);
}

void obs_pipewire_set_disconnected_callback(obs_pipewire *obs_pw, obs_pipewire_disconnected_cb callback,
					    void *user_data)
{
	pw_thread_loop_lock(obs_pw->thread_loop);
	obs_pw->disconnected_cb = callback;
	obs_pw->disconnected_data = user_data;

	/* A stream whose node goes away only pauses (obsproject/obs-studio#12063):
	 * watch the registry to notice. */
	if (callback && !obs_pw->node_registry) {
		obs_pw->node_registry = pw_core_get_registry(obs_pw->core, PW_VERSION_REGISTRY, 0);
		pw_registry_add_listener(obs_pw->node_registry, &obs_pw->node_registry_listener, &node_registry_events,
					 obs_pw);
	}
	pw_thread_loop_unlock(obs_pw->thread_loop);
}

bool obs_pipewire_is_disconnected(obs_pipewire *obs_pw)
{
	bool disconnected;

	if (!obs_pw)
		return false;

	/* Set on the PipeWire thread, under the thread loop lock. */
	pw_thread_loop_lock(obs_pw->thread_loop);
	disconnected = obs_pw->disconnected;
	pw_thread_loop_unlock(obs_pw->thread_loop);

	return disconnected;
}

void obs_pipewire_set_reconnecting(obs_pipewire *obs_pw, bool reconnecting)
{
	pw_thread_loop_lock(obs_pw->thread_loop);
	obs_pw->reconnecting = reconnecting;
	pw_thread_loop_unlock(obs_pw->thread_loop);
}

struct pw_registry *obs_pipewire_get_registry(obs_pipewire *obs_pw)
{
	return obs_pw->registry;
}

void obs_pipewire_roundtrip(obs_pipewire *obs_pw)
{
	pw_thread_loop_lock(obs_pw->thread_loop);

	obs_pw->sync_id = pw_core_sync(obs_pw->core, PW_ID_CORE, obs_pw->sync_id);
	pw_thread_loop_wait(obs_pw->thread_loop);

	pw_thread_loop_unlock(obs_pw->thread_loop);
}

void obs_pipewire_lock(obs_pipewire *obs_pw)
{
	if (obs_pw->thread_loop)
		pw_thread_loop_lock(obs_pw->thread_loop);
}

void obs_pipewire_unlock(obs_pipewire *obs_pw)
{
	if (obs_pw->thread_loop)
		pw_thread_loop_unlock(obs_pw->thread_loop);
}

void obs_pipewire_destroy(obs_pipewire *obs_pw)
{
	if (!obs_pw)
		return;

	while (obs_pw->streams->len > 0) {
		obs_pipewire_stream *obs_pw_stream = g_ptr_array_index(obs_pw->streams, 0);
		obs_pipewire_stream_destroy(obs_pw_stream);
	}
	g_clear_pointer(&obs_pw->streams, g_ptr_array_unref);
	teardown_pipewire(obs_pw);
	bfree(obs_pw);
}

obs_pipewire_stream *obs_pipewire_connect_stream(obs_pipewire *obs_pw, obs_source_t *source, int pipewire_node,
						 const struct obs_pipewire_connect_stream_info *connect_info)
{
	struct spa_pod_builder pod_builder;
	const struct spa_pod **params = NULL;
	obs_pipewire_stream *obs_pw_stream;
	uint32_t n_params;
	uint8_t params_buffer[4096];

	assert(connect_info != NULL);

	obs_pw_stream = bzalloc(sizeof(obs_pipewire_stream));
	obs_pw_stream->obs_pw = obs_pw;
	obs_pw_stream->source = source;
	obs_pw_stream->cursor.visible = connect_info->screencast.cursor_visible;
	obs_pw_stream->framerate.set = connect_info->video.framerate != NULL;
	obs_pw_stream->resolution.set = connect_info->video.resolution != NULL;
	obs_pw_frame_pacer_init(&obs_pw_stream->pacer);

	if (obs_pw_stream->framerate.set)
		obs_pw_stream->framerate.fraction = *connect_info->video.framerate;

	if (obs_pw_stream->resolution.set)
		obs_pw_stream->resolution.rect = *connect_info->video.resolution;

	init_format_info(obs_pw_stream, connect_info->video.format);

	pw_thread_loop_lock(obs_pw->thread_loop);

	/* Signal to renegotiate */
	obs_pw_stream->reneg =
		pw_loop_add_event(pw_thread_loop_get_loop(obs_pw->thread_loop), renegotiate_format, obs_pw_stream);
	blog(LOG_DEBUG, "[pipewire] registered event %p", obs_pw_stream->reneg);

	/* Stream */
	obs_pw_stream->stream = pw_stream_new(obs_pw->core, connect_info->stream_name, connect_info->stream_properties);
	pw_stream_add_listener(obs_pw_stream->stream, &obs_pw_stream->stream_listener, &stream_events, obs_pw_stream);
	blog(LOG_INFO, "[pipewire] Created stream %p", obs_pw_stream->stream);

	/* Stream parameters */
	pod_builder = SPA_POD_BUILDER_INIT(params_buffer, sizeof(params_buffer));

	obs_get_video_info(&obs_pw_stream->video_info);

	if (!build_format_params(obs_pw_stream, &pod_builder, &params, &n_params)) {
		pw_thread_loop_unlock(obs_pw->thread_loop);
		bfree(obs_pw_stream);
		return NULL;
	}

	pw_stream_connect(obs_pw_stream->stream, PW_DIRECTION_INPUT, pipewire_node,
			  PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS |
				  (connect_info->inactive ? PW_STREAM_FLAG_INACTIVE : 0),
			  params, n_params);
	obs_pw->watched_node = (uint32_t)pipewire_node;

	blog(LOG_INFO, "[pipewire] %s stream %p", connect_info->inactive ? "Paused" : "Playing", obs_pw_stream->stream);

	pw_thread_loop_unlock(obs_pw->thread_loop);
	bfree(params);

	g_ptr_array_add(obs_pw->streams, obs_pw_stream);

	return obs_pw_stream;
}

bool obs_pipewire_stream_has_frame(obs_pipewire_stream *obs_pw_stream)
{
	return obs_pw_stream && (obs_pw_stream->texture || obs_pw_stream->pacer.queued > 0);
}

bool obs_pipewire_stream_has_streamed(obs_pipewire_stream *obs_pw_stream)
{
	bool streamed;

	if (!obs_pw_stream)
		return false;

	/* Set on the PipeWire thread, under the thread loop lock. */
	pw_thread_loop_lock(obs_pw_stream->obs_pw->thread_loop);
	streamed = obs_pw_stream->streamed;
	pw_thread_loop_unlock(obs_pw_stream->obs_pw->thread_loop);

	return streamed;
}

void obs_pipewire_stream_show(obs_pipewire_stream *obs_pw_stream)
{
	if (obs_pw_stream->stream) {
		pw_thread_loop_lock(obs_pw_stream->obs_pw->thread_loop);
		pw_stream_set_active(obs_pw_stream->stream, true);
		pw_thread_loop_unlock(obs_pw_stream->obs_pw->thread_loop);
	}
}

void obs_pipewire_stream_hide(obs_pipewire_stream *obs_pw_stream)
{
	if (obs_pw_stream->stream) {
		pw_thread_loop_lock(obs_pw_stream->obs_pw->thread_loop);
		pw_stream_set_active(obs_pw_stream->stream, false);
		pw_thread_loop_unlock(obs_pw_stream->obs_pw->thread_loop);
	}
}

uint32_t obs_pipewire_stream_get_width(obs_pipewire_stream *obs_pw_stream)
{
	bool has_crop;

	if (!obs_pw_stream->negotiated)
		return 0;

	has_crop = has_effective_crop(obs_pw_stream);

	switch (obs_pw_stream->transform) {
	case SPA_META_TRANSFORMATION_Flipped:
	case SPA_META_TRANSFORMATION_None:
	case SPA_META_TRANSFORMATION_Flipped180:
	case SPA_META_TRANSFORMATION_180:
		return has_crop ? obs_pw_stream->crop.width : obs_pw_stream->format.info.raw.size.width;
	case SPA_META_TRANSFORMATION_Flipped90:
	case SPA_META_TRANSFORMATION_90:
	case SPA_META_TRANSFORMATION_Flipped270:
	case SPA_META_TRANSFORMATION_270:
		return has_crop ? obs_pw_stream->crop.height : obs_pw_stream->format.info.raw.size.height;
	default:
		return 0;
	}
}

uint32_t obs_pipewire_stream_get_height(obs_pipewire_stream *obs_pw_stream)
{
	bool has_crop;

	if (!obs_pw_stream->negotiated)
		return 0;

	has_crop = has_effective_crop(obs_pw_stream);

	switch (obs_pw_stream->transform) {
	case SPA_META_TRANSFORMATION_Flipped:
	case SPA_META_TRANSFORMATION_None:
	case SPA_META_TRANSFORMATION_Flipped180:
	case SPA_META_TRANSFORMATION_180:
		return has_crop ? obs_pw_stream->crop.height : obs_pw_stream->format.info.raw.size.height;
	case SPA_META_TRANSFORMATION_Flipped90:
	case SPA_META_TRANSFORMATION_90:
	case SPA_META_TRANSFORMATION_Flipped270:
	case SPA_META_TRANSFORMATION_270:
		return has_crop ? obs_pw_stream->crop.width : obs_pw_stream->format.info.raw.size.width;
	default:
		return 0;
	}
}

/* Moves on to the next captured frame, once per video tick however often
 * the source is drawn (preview, projectors, ...). */
static void advance_frame(obs_pipewire_stream *obs_pw_stream)
{
	uint64_t video_time = obs_get_video_frame_time();
	struct obs_pw_frame *frame;
	uint64_t released;
	uint64_t shown;

	if (video_time == obs_pw_stream->paced_video_time)
		return;
	obs_pw_stream->paced_video_time = video_time;

	shown = obs_pw_frame_pacer_tick(&obs_pw_stream->pacer, &released);
	if (shown != OBS_PW_FRAME_NONE) {
		frame = &obs_pw_stream->frames[shown];
		obs_pw_stream->texture = frame->texture;
		obs_pw_stream->crop = frame->crop;
		obs_pw_stream->transform = frame->transform;
	}
	release_frame(obs_pw_stream, released);
}

void obs_pipewire_stream_video_render(obs_pipewire_stream *obs_pw_stream, gs_effect_t *effect)
{
	bool rotated;
	int flip = 0;

	gs_eparam_t *image;

	advance_frame(obs_pw_stream);

	if (!obs_pw_stream->texture)
		return;

	/* FIXME: Use obs_pw_stream->format.info.raw colorimetry info to handle
	 * textures in their corresponding color space */
	image = gs_effect_get_param_by_name(effect, "image");
	gs_effect_set_texture(image, obs_pw_stream->texture);

	rotated = push_rotation(obs_pw_stream);

	flip = get_buffer_flip(obs_pw_stream);

	/* There is a SPA_VIDEO_FLAG_PREMULTIPLIED_ALPHA flag, but it does not
	 * seem to be fully implemented nor ever set. Just assume premultiplied
	 * always, which seems to be the default convention.
	 *
	 * See https://gitlab.freedesktop.org/pipewire/pipewire/-/issues/3126
	 *
	 * The cursor bitmap alpha mode is also not specified, and the
	 * convention there also seems to be premultiplied.
	 *
	 * Turn blending on as well: when the output colour space differs from
	 * the source's (Rec. 2100 PQ/HLG), libobs renders the source into an
	 * intermediate texture with blending disabled, and the cursor's
	 * transparent texels would replace the frame under them. */
	gs_blend_state_push();
	gs_enable_blending(true);
	gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

	if (has_effective_crop(obs_pw_stream)) {
		gs_draw_sprite_subregion(obs_pw_stream->texture, flip, obs_pw_stream->crop.x, obs_pw_stream->crop.y,
					 obs_pw_stream->crop.width, obs_pw_stream->crop.height);
	} else {
		gs_draw_sprite(obs_pw_stream->texture, flip, 0, 0);
	}

	if (rotated)
		gs_matrix_pop();

	if (obs_pw_stream->cursor.visible && obs_pw_stream->cursor.valid && obs_pw_stream->cursor.texture) {
		float cursor_x = obs_pw_stream->cursor.x - obs_pw_stream->cursor.hotspot_x;
		float cursor_y = obs_pw_stream->cursor.y - obs_pw_stream->cursor.hotspot_y;

		gs_matrix_push();
		gs_matrix_translate3f(cursor_x, cursor_y, 0.0f);

		gs_effect_set_texture(image, obs_pw_stream->cursor.texture);
		gs_draw_sprite(obs_pw_stream->texture, 0, obs_pw_stream->cursor.width, obs_pw_stream->cursor.height);

		gs_matrix_pop();
	}

	gs_blend_state_pop();
}

void obs_pipewire_stream_set_cursor_visible(obs_pipewire_stream *obs_pw_stream, bool cursor_visible)
{
	obs_pw_stream->cursor.visible = cursor_visible;
}

void obs_pipewire_stream_destroy(obs_pipewire_stream *obs_pw_stream)
{
	uint32_t output_flags;

	if (!obs_pw_stream)
		return;

	output_flags = obs_source_get_output_flags(obs_pw_stream->source);
	if (output_flags & OBS_SOURCE_ASYNC_VIDEO)
		obs_source_output_video(obs_pw_stream->source, NULL);

	g_ptr_array_remove(obs_pw_stream->obs_pw->streams, obs_pw_stream);

	obs_enter_graphics();
	g_clear_pointer(&obs_pw_stream->cursor.texture, gs_texture_destroy);
	obs_leave_graphics();

	pw_thread_loop_lock(obs_pw_stream->obs_pw->thread_loop);
	if (obs_pw_stream->stream)
		pw_stream_disconnect(obs_pw_stream->stream);
	g_clear_pointer(&obs_pw_stream->stream, pw_stream_destroy);
	pw_thread_loop_unlock(obs_pw_stream->obs_pw->thread_loop);

	/* No frame arrives any more */
	uint64_t released[OBS_PW_FRAME_PACER_MAX_FRAMES + 1];
	size_t count = obs_pw_frame_pacer_flush(&obs_pw_stream->pacer, released);

	obs_enter_graphics();
	for (size_t i = 0; i < count; i++)
		release_frame(obs_pw_stream, released[i]);
	for (size_t i = 0; i < OBS_PW_FRAME_SLOTS; i++)
		g_clear_pointer(&obs_pw_stream->frames[i].texrender, gs_texrender_destroy);
	obs_pw_stream->texture = NULL;
	obs_leave_graphics();

	clear_format_info(obs_pw_stream);
	bfree(obs_pw_stream);
}

void obs_pipewire_stream_set_framerate(obs_pipewire_stream *obs_pw_stream, const struct spa_fraction *framerate)
{
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;

	if ((!obs_pw_stream->framerate.set && !framerate) ||
	    (obs_pw_stream->framerate.set && framerate && obs_pw_stream->framerate.fraction.num == framerate->num &&
	     obs_pw_stream->framerate.fraction.denom == framerate->denom))
		return;

	/* A stream that already runs at this framerate keeps running: the
	 * camera portal asks again for the selected framerate whenever its
	 * properties are shown, and cameras behind libcamera cannot be
	 * reconfigured while they stream (obsproject/obs-studio#10399). */
	bool running_at_it = false;

	pw_thread_loop_lock(obs_pw->thread_loop);
	if (framerate) {
		running_at_it = obs_pw_stream->negotiated &&
				obs_pw_stream->format.info.raw.framerate.num == framerate->num &&
				obs_pw_stream->format.info.raw.framerate.denom == framerate->denom;
		obs_pw_stream->framerate.fraction = *framerate;
		obs_pw_stream->framerate.set = true;
	} else {
		obs_pw_stream->framerate.fraction = SPA_FRACTION(0, 0);
		obs_pw_stream->framerate.set = false;
	}
	pw_thread_loop_unlock(obs_pw->thread_loop);

	if (running_at_it)
		return;

	/* Signal to renegotiate */
	pw_loop_signal_event(pw_thread_loop_get_loop(obs_pw->thread_loop), obs_pw_stream->reneg);
}

void obs_pipewire_stream_set_resolution(obs_pipewire_stream *obs_pw_stream, const struct spa_rectangle *resolution)
{
	obs_pipewire *obs_pw = obs_pw_stream->obs_pw;

	if ((!obs_pw_stream->resolution.set && !resolution) ||
	    (obs_pw_stream->resolution.set && resolution && obs_pw_stream->resolution.rect.width == resolution->width &&
	     obs_pw_stream->resolution.rect.height == resolution->height))
		return;

	/* As for the framerate: a stream that already runs at this size keeps
	 * running. */
	bool running_at_it = false;

	pw_thread_loop_lock(obs_pw->thread_loop);
	if (resolution) {
		running_at_it = obs_pw_stream->negotiated &&
				obs_pw_stream->format.info.raw.size.width == resolution->width &&
				obs_pw_stream->format.info.raw.size.height == resolution->height;
		obs_pw_stream->resolution.rect = *resolution;
		obs_pw_stream->resolution.set = true;
	} else {
		obs_pw_stream->resolution.rect = SPA_RECTANGLE(0, 0);
		obs_pw_stream->resolution.set = false;
	}
	pw_thread_loop_unlock(obs_pw->thread_loop);

	if (running_at_it)
		return;

	/* Signal to renegotiate */
	pw_loop_signal_event(pw_thread_loop_get_loop(obs_pw->thread_loop), obs_pw_stream->reneg);
}
