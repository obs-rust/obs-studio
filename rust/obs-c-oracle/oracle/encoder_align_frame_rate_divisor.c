/* Immutable pre-swap C body on real encoder fields, compared with the Rust ABI.
 * Only the video-io effect is recorded; neither callback is executed. */
#include "obs-internal.h"
#include <string.h>

struct align_record {
	unsigned calls;
	video_t *media;
	uint64_t timestamp;
	void (*callback)(void *, struct video_data *);
	void *encoder;
};
struct align_fixture {
	struct obs_encoder encoder;
	struct align_record record;
	unsigned callback_calls;
	char media;
};

static void receive_video(void *param, struct video_data *frame)
{
	(void)frame;
	struct align_fixture *f = ((obs_encoder_t *)param)->context.data;
	f->callback_calls++;
}

static bool align_record_call(video_t *media, uint64_t timestamp, void (*callback)(void *, struct video_data *),
			      void *param)
{
	struct align_fixture *f = ((obs_encoder_t *)param)->context.data;
	f->record.calls++;
	f->record.media = media;
	f->record.timestamp = timestamp;
	f->record.callback = callback;
	f->record.encoder = param;
	return true;
}

#define video_output_align_frame_rate_divisor align_record_call
void original_encoder_align_frame_rate_divisor(obs_encoder_t *encoder, uint64_t timestamp);
#define obs_encoder_align_frame_rate_divisor original_encoder_align_frame_rate_divisor
#include "encoder_align_frame_rate_divisor_original.inc"
#undef obs_encoder_align_frame_rate_divisor
#include "encoder_align_frame_rate_divisor_fields.inc"
#undef video_output_align_frame_rate_divisor

typedef void (*align_operation)(obs_encoder_t *, uint64_t, void (*)(void *, struct video_data *));
bool oracle_encoder_align_probe(bool present, bool media, uint32_t divisor, uint64_t timestamp, align_operation rust);
bool oracle_encoder_align_probe(bool present, bool media, uint32_t divisor, uint64_t timestamp, align_operation rust)
{
	struct align_fixture f;
	memset(&f, 0, sizeof(f));
	f.encoder.media = media ? &f.media : NULL;
	f.encoder.frame_rate_divisor = divisor;
	f.encoder.context.data = &f;
	f.encoder.scaled_width = ~divisor;
	struct obs_encoder before;
	memcpy(&before, &f.encoder, sizeof(before));
	obs_encoder_t *encoder = present ? &f.encoder : NULL;
	original_encoder_align_frame_rate_divisor(encoder, timestamp);
	struct align_record original;
	memcpy(&original, &f.record, sizeof(original));
	bool unchanged = memcmp(&before, &f.encoder, sizeof(before)) == 0;
	memset(&f.record, 0, sizeof(f.record));
	rust(encoder, timestamp, receive_video);
	return unchanged && memcmp(&original, &f.record, sizeof(original)) == 0 &&
	       memcmp(&before, &f.encoder, sizeof(before)) == 0 && f.callback_calls == 0;
}
