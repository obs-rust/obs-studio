#include "media-time.h"

#include <stdio.h>

/* Stub: the arithmetic MediaControls uses today. The elapsed label is always
 * derived from the slider position, in float. */

int media_time_to_slider(int64_t time_ms, int64_t duration_ms, int slider_max)
{
	float time = (float)time_ms;
	float duration = (float)duration_ms;

	if (!duration)
		return 0;
	return (int)((time / duration) * (float)slider_max);
}

int64_t media_time_from_slider(int slider_value, int slider_max, int64_t duration_ms)
{
	float percent = (float)slider_value / (float)slider_max;

	return (int64_t)(percent * (float)duration_ms);
}

static void format_seconds(char *buf, size_t size, const char *prefix, int total_seconds)
{
	int seconds = total_seconds % 60;
	int total_minutes = total_seconds / 60;
	int minutes = total_minutes % 60;
	int hours = total_minutes / 60;

	snprintf(buf, size, "%s%02d:%02d:%02d", prefix, hours, minutes, seconds);
}

void media_time_format(char *buf, size_t size, int64_t time_ms)
{
	format_seconds(buf, size, "", (int)(time_ms / 1000));
}

void media_time_get_labels(struct media_time_labels *labels, int64_t time_ms, int64_t duration_ms, bool seeking,
			   int slider_value, int slider_max, bool countdown)
{
	int val = seeking ? slider_value : media_time_to_slider(time_ms, duration_ms, slider_max);
	float duration = (float)duration_ms;
	float percent = (float)val / (float)slider_max;
	float time = percent * duration;

	format_seconds(labels->elapsed, sizeof(labels->elapsed), "", (int)(time / 1000.0f));
	if (!countdown)
		format_seconds(labels->duration, sizeof(labels->duration), "", (int)(duration / 1000.0f));
	else
		format_seconds(labels->duration, sizeof(labels->duration), "-", (int)((duration - time) / 1000.0f));
}
