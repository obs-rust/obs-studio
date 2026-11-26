#include "libobs/build/libobs.pch.h"

void obs_source_output_video(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (destroying(source))
		return;
	if (!frame) {
		obs_source_output_video_internal(source, NULL);
		return;
	}

	struct obs_source_frame new_frame = *frame;
	new_frame.full_range = obs_source_frame_copy_full_range((int)frame->format, new_frame.full_range);

	obs_source_output_video_internal(source, &new_frame);
}

void obs_source_output_video2(obs_source_t *source, const struct obs_source_frame2 *frame)
{
	if (destroying(source))
		return;
	if (!frame) {
		obs_source_output_video_internal(source, NULL);
		return;
	}

	struct obs_source_frame new_frame = {0};

	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		new_frame.data[i] = frame->data[i];
		new_frame.linesize[i] = frame->linesize[i];
	}

	new_frame.width = frame->width;
	new_frame.height = frame->height;
	new_frame.timestamp = frame->timestamp;
	new_frame.format = frame->format;
	new_frame.full_range = obs_source_frame_copy_full_range2((int)frame->format, (int)frame->range);
	new_frame.max_luminance = 0;
	new_frame.flip = frame->flip;
	new_frame.flags = frame->flags;
	new_frame.trc = frame->trc;

	memcpy(&new_frame.color_matrix, &frame->color_matrix, sizeof(frame->color_matrix));
	memcpy(&new_frame.color_range_min, &frame->color_range_min, sizeof(frame->color_range_min));
	memcpy(&new_frame.color_range_max, &frame->color_range_max, sizeof(frame->color_range_max));

	obs_source_output_video_internal(source, &new_frame);
}

void obs_source_preload_video(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (destroying(source))
		return;
	if (!frame) {
		obs_source_preload_video_internal(source, NULL);
		return;
	}

	struct obs_source_frame new_frame = *frame;
	new_frame.full_range = obs_source_frame_copy_full_range((int)frame->format, new_frame.full_range);

	obs_source_preload_video_internal(source, &new_frame);
}

void obs_source_preload_video2(obs_source_t *source, const struct obs_source_frame2 *frame)
{
	if (destroying(source))
		return;
	if (!frame) {
		obs_source_preload_video_internal(source, NULL);
		return;
	}

	struct obs_source_frame new_frame = {0};

	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		new_frame.data[i] = frame->data[i];
		new_frame.linesize[i] = frame->linesize[i];
	}

	new_frame.width = frame->width;
	new_frame.height = frame->height;
	new_frame.timestamp = frame->timestamp;
	new_frame.format = frame->format;
	new_frame.full_range = obs_source_frame_copy_full_range2((int)frame->format, (int)frame->range);
	new_frame.max_luminance = 0;
	new_frame.flip = frame->flip;
	new_frame.flags = frame->flags;
	new_frame.trc = frame->trc;

	memcpy(&new_frame.color_matrix, &frame->color_matrix, sizeof(frame->color_matrix));
	memcpy(&new_frame.color_range_min, &frame->color_range_min, sizeof(frame->color_range_min));
	memcpy(&new_frame.color_range_max, &frame->color_range_max, sizeof(frame->color_range_max));

	obs_source_preload_video_internal(source, &new_frame);
}

void obs_source_set_video_frame(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (destroying(source))
		return;
	if (!frame) {
		obs_source_preload_video_internal(source, NULL);
		return;
	}

	struct obs_source_frame new_frame = *frame;
	new_frame.full_range = obs_source_frame_copy_full_range((int)frame->format, new_frame.full_range);

	obs_source_set_video_frame_internal(source, &new_frame);
}

void obs_source_set_video_frame2(obs_source_t *source, const struct obs_source_frame2 *frame)
{
	if (destroying(source))
		return;
	if (!frame) {
		obs_source_preload_video_internal(source, NULL);
		return;
	}

	struct obs_source_frame new_frame = {0};

	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		new_frame.data[i] = frame->data[i];
		new_frame.linesize[i] = frame->linesize[i];
	}

	new_frame.width = frame->width;
	new_frame.height = frame->height;
	new_frame.timestamp = frame->timestamp;
	new_frame.format = frame->format;
	new_frame.full_range = obs_source_frame_copy_full_range2((int)frame->format, (int)frame->range);
	new_frame.max_luminance = 0;
	new_frame.flip = frame->flip;
	new_frame.flags = frame->flags;
	new_frame.trc = frame->trc;

	memcpy(&new_frame.color_matrix, &frame->color_matrix, sizeof(frame->color_matrix));
	memcpy(&new_frame.color_range_min, &frame->color_range_min, sizeof(frame->color_range_min));
	memcpy(&new_frame.color_range_max, &frame->color_range_max, sizeof(frame->color_range_max));

	obs_source_set_video_frame_internal(source, &new_frame);
}
