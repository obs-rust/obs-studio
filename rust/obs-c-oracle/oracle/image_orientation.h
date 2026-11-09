/* Test doubles only for FFmpeg access; selection is the verbatim C extract.
 * The modern converter is a deterministic stand-in, not FFmpeg 8.1 coverage. */
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
	const char *key;
	const char *value;
} AVDictionaryEntry;
typedef struct {
	const uint8_t *data;
	size_t size;
} AVFrameSideData;
typedef struct {
	AVDictionaryEntry *metadata;
	const AVFrameSideData *side;
} AVFrame;
#define AV_DICT_MATCH_CASE 1
#define AV_FRAME_DATA_DISPLAYMATRIX 6

#if !FFMPEG_HAS_EXIF_API
static AVDictionaryEntry *av_dict_get(AVDictionaryEntry *metadata, const char *key, const AVDictionaryEntry *previous,
				      int flags)
{
	(void)previous;
	(void)flags;
	return metadata && metadata->key && strcmp(metadata->key, key) == 0 ? metadata : NULL;
}
extern int oracle_gs_ffmpeg_image_matrix_orientation(int32_t a, int32_t b, int32_t c, int32_t d);
#define gs_ffmpeg_image_matrix_orientation oracle_gs_ffmpeg_image_matrix_orientation
#else
static int av_exif_matrix_to_orientation(const int32_t *matrix)
{
	uint32_t result = 0;
	for (size_t i = 0; i < 9; i++) {
		result = result * 31u + (uint32_t)matrix[i];
	}
	return (int)(result % 9u);
}
#endif

static const AVFrameSideData *av_frame_get_side_data(const AVFrame *frame, int type)
{
	(void)type;
	return frame->side;
}

#include "image-orientation.inc"

int ORIENTATION_ORACLE(const char *key, const char *value, const uint8_t *data, size_t size, int present);
int ORIENTATION_ORACLE(const char *key, const char *value, const uint8_t *data, size_t size, int present)
{
	AVDictionaryEntry metadata = {key, value};
	AVFrameSideData side = {data, size};
	AVFrame frame = {&metadata, present ? &side : NULL};
	return ffmpeg_image_get_orientation(&frame);
}
