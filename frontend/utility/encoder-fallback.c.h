#include "frontend/build/frontend-utility-encoder-fallback.pch.h"

const char *adv_video_encoder_fallback(const char *id)
{
	if (!id) {
		return "obs_x264";
	}
	if (strcmp(id, "obs_nvenc_hevc_tex") == 0 || strcmp(id, "obs_nvenc_av1_tex") == 0 ||
	    strcmp(id, "jim_hevc_nvenc") == 0 || strcmp(id, "jim_av1_nvenc") == 0) {
		return "obs_nvenc_h264_tex";
	}
	if (strcmp(id, "h265_texture_amf") == 0 || strcmp(id, "av1_texture_amf") == 0) {
		return "h264_texture_amf";
	}
	if (strcmp(id, "com.apple.videotoolbox.videoencoder.ave.hevc") == 0) {
		return "com.apple.videotoolbox.videoencoder.ave.avc";
	}
	if (strcmp(id, "obs_qsv11_av1") == 0) {
		return "obs_qsv11";
	}
	return "obs_x264";
}

const char *encoder_list_select(const char *configured, const char *const *fallbacks, encoder_list_has_fn has,
				void *param)
{
	if (configured && has(configured, param)) {
		return configured;
	}

	for (const char *const *fb = fallbacks; fb && *fb; fb++) {
		if (has(*fb, param)) {
			return *fb;
		}
	}
	return NULL;
}
