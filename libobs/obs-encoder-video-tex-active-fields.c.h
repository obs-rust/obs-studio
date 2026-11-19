#include "libobs/build/libobs.pch.h"

const struct obs_core_video_mix *obs_encoder_video_tex_active_mix(const obs_encoder_t *encoder);
bool obs_encoder_video_tex_active_nv12(const struct obs_core_video_mix *mix);
bool obs_encoder_video_tex_active_p010(const struct obs_core_video_mix *mix);

const struct obs_core_video_mix *obs_encoder_video_tex_active_mix(const obs_encoder_t *encoder)
{
	return get_mix_for_video(encoder->media);
}

bool obs_encoder_video_tex_active_nv12(const struct obs_core_video_mix *mix)
{
	return mix->using_nv12_tex;
}

bool obs_encoder_video_tex_active_p010(const struct obs_core_video_mix *mix)
{
	return mix->using_p010_tex;
}
