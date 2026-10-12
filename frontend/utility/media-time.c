#include "media-time.h"

#include <stdio.h>

static int64_t clamp_time(int64_t time_ms, int64_t duration_ms)
{
	if (time_ms < 0 || duration_ms <= 0)
		return 0;
	return time_ms > duration_ms ? duration_ms : time_ms;
}

int media_time_to_slider(int64_t time_ms, int64_t duration_ms, int slider_max)
{
	if (duration_ms <= 0 || slider_max <= 0)
		return 0;

	time_ms = clamp_time(time_ms, duration_ms);
	return (int)(time_ms * slider_max / duration_ms);
}

int64_t media_time_from_slider(int slider_value, int slider_max, int64_t duration_ms)
{
	if (duration_ms <= 0 || slider_max <= 0 || slider_value <= 0)
		return 0;
	if (slider_value >= slider_max)
		return duration_ms;

	return duration_ms * slider_value / slider_max;
}

void media_time_format(char *buf, size_t size, int64_t time_ms)
{
	long long total_seconds = time_ms > 0 ? (long long)(time_ms / 1000) : 0;
	long long hours = total_seconds / 3600;
	int minutes = (int)(total_seconds / 60 % 60);
	int seconds = (int)(total_seconds % 60);

	snprintf(buf, size, "%02lld:%02d:%02d", hours, minutes, seconds);
}

void media_time_get_labels(struct media_time_labels *labels, int64_t time_ms, int64_t duration_ms, bool seeking,
			   int slider_value, int slider_max, bool countdown)
{
	int64_t elapsed;

	if (duration_ms < 0)
		duration_ms = 0;

	/* The slider has only slider_max steps, so it is used for the label
	 * only while the user drags it. Otherwise the label shows the media's
	 * own time, which moves every second whatever the duration. */
	if (seeking)
		elapsed = media_time_from_slider(slider_value, slider_max, duration_ms);
	else
		elapsed = clamp_time(time_ms, duration_ms);

	media_time_format(labels->elapsed, sizeof(labels->elapsed), elapsed);

	if (!countdown) {
		media_time_format(labels->duration, sizeof(labels->duration), duration_ms);
	} else {
		labels->duration[0] = '-';
		media_time_format(labels->duration + 1, sizeof(labels->duration) - 1, duration_ms - elapsed);
	}
}
