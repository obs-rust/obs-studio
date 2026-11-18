#include "libobs/build/libobs-media.pch.h"

/* Private C/Rust session ABI, deliberately opaque outside this file. */
void *obs_texture_file_data_open(const char *file);
void *obs_texture_file_data_decode(void *session);
int obs_texture_file_data_format(void *session);
uint32_t obs_texture_file_data_width(void *session);
uint32_t obs_texture_file_data_height(void *session);
void obs_texture_file_data_close(void *session);

/* Private opaque session: FFmpeg's layout never crosses the Rust boundary.
 * Initialization already releases FFmpeg resources on failure. */
void *obs_texture_file_data_open(const char *file)
{
	struct ffmpeg_image *image = bzalloc(sizeof(*image));
	if (!ffmpeg_image_init(image, file, gs_image_max_texture_size)) {
		bfree(image);
		return NULL;
	}
	return image;
}

void *obs_texture_file_data_decode(void *session)
{
	struct gs_image_decode_context decode = {session, ffmpeg_image_too_large, ffmpeg_image_reformat_frame};
	return gs_ffmpeg_image_decode(&decode, (int)GS_IMAGE_ALPHA_STRAIGHT);
}

int obs_texture_file_data_format(void *session)
{
	const struct ffmpeg_image *image = session;
	return gs_ffmpeg_image_color_format(image->format);
}

uint32_t obs_texture_file_data_width(void *session)
{
	return (uint32_t)((struct ffmpeg_image *)session)->cx;
}

uint32_t obs_texture_file_data_height(void *session)
{
	return (uint32_t)((struct ffmpeg_image *)session)->cy;
}

void obs_texture_file_data_close(void *session)
{
	ffmpeg_image_free(session);
	bfree(session);
}
