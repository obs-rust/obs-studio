#include "libobs/build/libobs-media.pch.h"

const char *gs_image_orientation_metadata(const void *opaque)
{
#if FFMPEG_HAS_EXIF_API
	(void)opaque;
	return NULL;
#else
	const AVFrame *frame = opaque;
	AVDictionaryEntry *entry = av_dict_get(frame->metadata, "Orientation", NULL, AV_DICT_MATCH_CASE);
	return entry ? entry->value : NULL;
#endif
}

const uint8_t *gs_image_orientation_matrix(const void *opaque, size_t *size)
{
	const AVFrameSideData *sd = av_frame_get_side_data(opaque, AV_FRAME_DATA_DISPLAYMATRIX);
	*size = sd ? sd->size : 0;
	return sd ? sd->data : NULL;
}

int gs_image_orientation_convert(const int32_t *matrix)
{
#if FFMPEG_HAS_EXIF_API
	return av_exif_matrix_to_orientation(matrix);
#else
	return gs_ffmpeg_image_matrix_orientation(matrix[0], matrix[1], matrix[3], matrix[4]);
#endif
}
