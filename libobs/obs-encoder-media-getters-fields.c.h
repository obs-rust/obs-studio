#include "libobs/build/libobs.pch.h"

struct obs_encoder_media_getters_fields {
	int type;
	void *media;
	video_t *fps_override;
	const char *name;
};

struct obs_encoder_media_getters_fields obs_encoder_media_getters_fields(const obs_encoder_t *encoder);
struct obs_encoder_media_getters_fields obs_encoder_media_getters_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_media_getters_fields){(int)encoder->info.type, encoder->media, encoder->fps_override,
							 encoder->context.name};
}
