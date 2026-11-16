/* Real source layout and original C allocation routine. No behavior exclusions
 * except channel counts above MAX_AV_PLANES, which index outside C arrays. */
#include "obs-internal.h"
#include <stddef.h>
#include <string.h>

_Static_assert(MAX_AV_PLANES == 8, "update fixture input/output arrays with plane capacity");

struct texture_capture {
	uint64_t *out;
	obs_source_t *source;
	int fail_plane;
	bool fail_render;
	unsigned count;
	unsigned char handles[MAX_AV_PLANES], render_handle;
	/* When set, the channel count each allocation leaves behind (0 to
	 * MAX_AV_PLANES), indexed by allocation: the render first, then each
	 * plane. */
	const int *counts;
	unsigned events;
};

/* A caller whose allocation changes the source: the count and the next
 * plane change after each allocation, so a read made before it shows. */
static void texture_mutate(struct texture_capture *s, unsigned next)
{
	if (!s->counts)
		return;
	s->source->async_channel_count = s->counts[s->events++];
	if (next < MAX_AV_PLANES) {
		s->source->async_convert_width[next] ^= 0x5555u;
		s->source->async_convert_height[next] += 3;
		s->source->async_texture_formats[next] =
			(enum gs_color_format)((int)s->source->async_texture_formats[next] + 1);
	}
}
#if defined(_MSC_VER)
static __declspec(thread) struct texture_capture *texture_current;
#else
static _Thread_local struct texture_capture *texture_current;
#endif

static gs_texrender_t *texture_record_render(enum gs_color_format format, enum gs_zstencil_format zs)
{
	struct texture_capture *s = texture_current;
	s->out[0]++;
	s->out[1] = (uint32_t)format;
	s->out[2] = (uint32_t)zs;
	s->out[3] = s->count;
	texture_mutate(s, 0);
	return s->fail_render ? NULL : (gs_texrender_t *)&s->render_handle;
}
static gs_texture_t *texture_record_create(uint32_t width, uint32_t height, enum gs_color_format format,
					   uint32_t levels, const uint8_t **data, uint32_t flags)
{
	struct texture_capture *s = texture_current;
	unsigned c = s->count++;
	uint64_t *out = s->out + 4 + c * 8;
	out[0] = width;
	out[1] = height;
	out[2] = (uint32_t)format;
	out[3] = levels;
	out[4] = data != NULL;
	out[5] = flags;
	out[6] = s->out[0];
	/* Observe immediate stores, rather than only the final destinations. */
	out[7] = c ? s->source->async_prev_textures[c - 1] ==
				 ((int)(c - 1) == s->fail_plane ? NULL : (gs_texture_t *)&s->handles[c - 1])
		   : true;
	if (s->source->async_gpu_conversion)
		out[7] = out[7] && s->source->async_prev_texrender ==
					   (s->fail_render ? NULL : (gs_texrender_t *)&s->render_handle);
	texture_mutate(s, c + 1);
	return (int)c == s->fail_plane ? NULL : (gs_texture_t *)&s->handles[c];
}
void oracle_set_deinterlace_texture_size(obs_source_t *source);

#define gs_texture_create texture_record_create
#define gs_texrender_create texture_record_render
#define set_deinterlace_texture_size oracle_set_deinterlace_texture_size
#include "obs-source-deinterlace-texture.inc"
#undef set_deinterlace_texture_size
#include "source-deinterlace-texture-fields.inc"
#undef gs_texrender_create
#undef gs_texture_create

void oracle_deinterlace_texture_run(const uint32_t *values, const int *counts, int fail_plane, bool fail_render,
				    void (*run)(obs_source_t *,
						gs_texture_t *(*)(uint32_t, uint32_t, enum gs_color_format, uint32_t,
								  const uint8_t **, uint32_t),
						gs_texrender_t *(*)(enum gs_color_format, enum gs_zstencil_format)),
				    uint64_t *out);
void oracle_deinterlace_texture_run(const uint32_t *values, const int *counts, int fail_plane, bool fail_render,
				    void (*run)(obs_source_t *,
						gs_texture_t *(*)(uint32_t, uint32_t, enum gs_color_format, uint32_t,
								  const uint8_t **, uint32_t),
						gs_texrender_t *(*)(enum gs_color_format, enum gs_zstencil_format)),
				    uint64_t *out)
{
	obs_source_t source = {0};
	source.async_format = (enum video_format)values[0];
	source.async_trc = (uint8_t)values[1];
	source.async_gpu_conversion = values[2] != 0;
	source.async_channel_count = (int)values[3];
	source.async_width = values[4];
	source.async_height = values[5];
	source.async_prev_texrender = (gs_texrender_t *)&source;
	for (unsigned c = 0; c < MAX_AV_PLANES; c++) {
		source.async_convert_width[c] = values[6 + c];
		source.async_convert_height[c] = values[14 + c];
		source.async_texture_formats[c] = (enum gs_color_format)values[22 + c];
		source.async_prev_textures[c] = (gs_texture_t *)&source;
	}
	unsigned char snapshot[sizeof(source)];
	memcpy(snapshot, &source, sizeof(source));
	memset(out, 0, 79 * sizeof(*out));
	struct texture_capture capture = {.out = out,
					  .source = &source,
					  .fail_plane = fail_plane,
					  .fail_render = fail_render,
					  .counts = counts};
	texture_current = &capture;
	if (run)
		run(&source, texture_record_create, texture_record_render);
	else
		oracle_set_deinterlace_texture_size(&source);
	texture_current = NULL;
	out[68] = capture.count;
	out[69] = source.async_prev_texrender == (gs_texrender_t *)&source                  ? 99
		  : source.async_prev_texrender == NULL                                     ? 0
		  : source.async_prev_texrender == (gs_texrender_t *)&capture.render_handle ? 1
											    : 100;
	for (unsigned c = 0; c < MAX_AV_PLANES; c++) {
		out[70 + c] = source.async_prev_textures[c] == (gs_texture_t *)&source               ? 99
			      : source.async_prev_textures[c] == NULL                                ? 0
			      : source.async_prev_textures[c] == (gs_texture_t *)&capture.handles[c] ? c + 1
												     : 100;
		source.async_prev_textures[c] = (gs_texture_t *)&source;
	}
	source.async_prev_texrender = (gs_texrender_t *)&source;
	out[78] = memcmp(snapshot, &source, sizeof(source)) == 0;
}

void oracle_deinterlace_texture_layout(size_t *out);
void oracle_deinterlace_texture_layout(size_t *out)
{
	out[0] = sizeof(struct obs_source_deinterlace_texture_fields);
	out[1] = _Alignof(struct obs_source_deinterlace_texture_fields);
#define TEXTURE_OFFSET(index, field) out[index] = offsetof(struct obs_source_deinterlace_texture_fields, field)
	TEXTURE_OFFSET(2, format);
	TEXTURE_OFFSET(3, trc);
	TEXTURE_OFFSET(4, gpu);
	TEXTURE_OFFSET(5, channels);
	TEXTURE_OFFSET(6, width);
	TEXTURE_OFFSET(7, height);
	TEXTURE_OFFSET(8, widths);
	TEXTURE_OFFSET(9, heights);
	TEXTURE_OFFSET(10, formats);
	TEXTURE_OFFSET(11, textures);
	TEXTURE_OFFSET(12, render);
#undef TEXTURE_OFFSET
	out[13] = MAX_AV_PLANES;
	out[14] = GS_DYNAMIC;
	out[15] = GS_ZS_NONE;
}
