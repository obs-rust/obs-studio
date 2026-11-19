#include "libobs/build/libobs.pch.h"

bool obs_encoder_video_tex_active(const obs_encoder_t *encoder, enum video_format format)
{
	/* No mix owns the video of an encoder with no video set, or of one
	 * whose canvas mix is gone (#2267): no texture input then. */
	const struct obs_core_video_mix *mix = get_mix_for_video(encoder->media);
	if (!mix)
		return false;

	if (format == VIDEO_FORMAT_NV12)
		return mix->using_nv12_tex;
	if (format == VIDEO_FORMAT_P010)
		return mix->using_p010_tex;

	return false;
}
