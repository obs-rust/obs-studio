#include "libobs/build/libobs.pch.h"

/* obs_source_get_color_space(), moved verbatim out of obs-source.c.h. The
 * Rust port (obs-source get_color_space, #5724) exports the symbol; this
 * copy builds only in the reference-only group and the Tier 3 oracle. */

/* Original private helper, retained for the standalone C reference/oracle. */
static inline bool data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}

enum gs_color_space obs_source_get_color_space(obs_source_t *source, size_t count,
					       const enum gs_color_space *preferred_spaces)
{
	if (!data_valid(source, "obs_source_get_color_space"))
		return GS_CS_SRGB;

	const struct obs_source_color_space_node node = {
		.type = (int)source->info.type,
		.output_flags = source->info.output_flags,
		.has_data = source->context.data != NULL,
		.enabled = source->enabled,
		.has_filter_parent = source->filter_parent != NULL,
		.has_filter_target = source->filter_target != NULL,
	};

	switch (obs_source_color_space_route(node)) {
	case OBS_SOURCE_COLOR_SPACE_ASK_PARENT:
		return obs_source_get_color_space(source->filter_parent, count, preferred_spaces);
	case OBS_SOURCE_COLOR_SPACE_ASK_TARGET:
		return obs_source_get_color_space(source->filter_target, count, preferred_spaces);
	case OBS_SOURCE_COLOR_SPACE_ASYNC_PICK: {
		const enum gs_color_space video_space = convert_video_space(source->async_format, source->async_trc);
		return (enum gs_color_space)obs_source_color_space_pick((int)video_space, count,
									(const int *)preferred_spaces);
	}
	case OBS_SOURCE_COLOR_SPACE_CALLBACK:
		break;
	}

	assert(source->context.data);
	return source->info.video_get_color_space
		       ? source->info.video_get_color_space(source->context.data, count, preferred_spaces)
		       : GS_CS_SRGB;
}
