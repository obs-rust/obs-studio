#include "libobs/build/libobs.pch.h"

void obs_source_frame_init(struct obs_source_frame *frame, enum video_format format, uint32_t width, uint32_t height)
{
	struct video_frame vid_frame;

	if (!obs_ptr_valid(frame, "obs_source_frame_init"))
		return;

	video_frame_init(&vid_frame, format, width, height);
	frame->format = format;
	frame->width = width;
	frame->height = height;

	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		frame->data[i] = vid_frame.data[i];
		frame->linesize[i] = vid_frame.linesize[i];
	}
}
