#include "libobs/build/libobs-media.pch.h"

/* FFmpeg 8.1 (libavcodec 62.13.100) parses the EXIF block of JPEG, PNG and
 * WebP images itself (the av_exif_* API). It exports the orientation only as
 * display matrix side data and removes the "Orientation" tag from the frame
 * metadata, where FFmpeg 8.0 and earlier put it (JPEG and WebP only).
 * Once the minimum FFmpeg is 8.1, the metadata tag and the sign table below
 * can go. */
#if !FFMPEG_HAS_EXIF_API
static int display_matrix_to_orientation(const int32_t *matrix)
{
	return gs_ffmpeg_image_matrix_orientation(matrix[0], matrix[1], matrix[3], matrix[4]);
}
#endif

/* EXIF orientation (1-8) of a decoded image, 0 if it has none. */
static int ffmpeg_image_get_orientation(const AVFrame *frame)
{
#if !FFMPEG_HAS_EXIF_API
	AVDictionaryEntry *entry = av_dict_get(frame->metadata, "Orientation", NULL, AV_DICT_MATCH_CASE);
	if (entry && entry->value)
		return atoi(entry->value);
#endif

	const AVFrameSideData *sd = av_frame_get_side_data(frame, AV_FRAME_DATA_DISPLAYMATRIX);
	if (!sd || sd->size < 9 * sizeof(int32_t))
		return 0;

	int32_t matrix[9];
	memcpy(matrix, sd->data, sizeof(matrix));
#if FFMPEG_HAS_EXIF_API
	return av_exif_matrix_to_orientation(matrix);
#else
	return display_matrix_to_orientation(matrix);
#endif
}
