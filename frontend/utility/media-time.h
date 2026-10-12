#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Time labels and slider positions for the Media Source controls, kept free
 * of Qt so they can be unit tested. Times are in milliseconds, as returned by
 * obs_source_media_get_time() and obs_source_media_get_duration(). */

#define MEDIA_TIME_LABEL_SIZE 32

struct media_time_labels {
	char elapsed[MEDIA_TIME_LABEL_SIZE];
	char duration[MEDIA_TIME_LABEL_SIZE];
};

/* Slider position (0 to slider_max) for a media time. */
int media_time_to_slider(int64_t time_ms, int64_t duration_ms, int slider_max);

/* Media time a slider position stands for. */
int64_t media_time_from_slider(int slider_value, int slider_max, int64_t duration_ms);

/* Formats a time as HH:MM:SS. Hours are not wrapped. */
void media_time_format(char *buf, size_t size, int64_t time_ms);

/* Fills the elapsed and duration labels. While the user drags the slider
 * (seeking), the elapsed label previews the slider position; otherwise it
 * shows the media's own time. With countdown, the duration label shows the
 * remaining time instead, prefixed with '-'. */
void media_time_get_labels(struct media_time_labels *labels, int64_t time_ms, int64_t duration_ms, bool seeking,
			   int slider_value, int slider_max, bool countdown);

#ifdef __cplusplus
}
#endif
