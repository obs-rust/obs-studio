#include "libobs/build/libobs.pch.h"

void obs_source_send_mouse_click(obs_source_t *source, const struct obs_mouse_event *event, int32_t type, bool mouse_up,
				 uint32_t click_count)
{
	if (!obs_source_valid(source, "obs_source_send_mouse_click"))
		return;

	if (source->info.output_flags & OBS_SOURCE_INTERACTION) {
		if (source->info.mouse_click) {
			source->info.mouse_click(source->context.data, event, type, mouse_up, click_count);
		}
	}
}

void obs_source_send_mouse_move(obs_source_t *source, const struct obs_mouse_event *event, bool mouse_leave)
{
	if (!obs_source_valid(source, "obs_source_send_mouse_move"))
		return;

	if (source->info.output_flags & OBS_SOURCE_INTERACTION) {
		if (source->info.mouse_move) {
			source->info.mouse_move(source->context.data, event, mouse_leave);
		}
	}
}

void obs_source_send_mouse_wheel(obs_source_t *source, const struct obs_mouse_event *event, int x_delta, int y_delta)
{
	if (!obs_source_valid(source, "obs_source_send_mouse_wheel"))
		return;

	if (source->info.output_flags & OBS_SOURCE_INTERACTION) {
		if (source->info.mouse_wheel) {
			source->info.mouse_wheel(source->context.data, event, x_delta, y_delta);
		}
	}
}

void obs_source_send_focus(obs_source_t *source, bool focus)
{
	if (!obs_source_valid(source, "obs_source_send_focus"))
		return;

	if (source->info.output_flags & OBS_SOURCE_INTERACTION) {
		if (source->info.focus) {
			source->info.focus(source->context.data, focus);
		}
	}
}

void obs_source_send_key_click(obs_source_t *source, const struct obs_key_event *event, bool key_up)
{
	if (!obs_source_valid(source, "obs_source_send_key_click"))
		return;

	if (source->info.output_flags & OBS_SOURCE_INTERACTION) {
		if (source->info.key_click) {
			source->info.key_click(source->context.data, event, key_up);
		}
	}
}
