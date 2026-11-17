#define FFMPEG_HAS_EXIF_API 0
#define ORIENTATION_ORACLE oracle_image_orientation_legacy
#include "image_orientation.h"

#include "graphics/graphics-ffmpeg-orientation.h"
#include "image-orientation-fields.inc"

int oracle_image_orientation_through_ffi(const char *key, const char *value, const uint8_t *data, size_t size,
					 int present, int (*select)(const void *));
int oracle_image_orientation_through_ffi(const char *key, const char *value, const uint8_t *data, size_t size,
					 int present, int (*select)(const void *))
{
	AVDictionaryEntry metadata = {key, value};
	AVFrameSideData side = {data, size};
	AVFrame frame = {&metadata, present ? &side : NULL};
	return select(&frame);
}
