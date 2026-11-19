/* Original query, unchanged mutex walk and real private field adapters. */
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

struct encoder_tex_fixture {
	struct obs_core *core;
	video_t *media;
	bool flags[3][2];
	unsigned lookups;
	bool lookup_ok;
};
#if defined(_MSC_VER)
#define ENCODER_TEX_THREAD_LOCAL __declspec(thread)
#else
#define ENCODER_TEX_THREAD_LOCAL __thread
#endif
static ENCODER_TEX_THREAD_LOCAL struct encoder_tex_fixture *encoder_tex_current;
#undef ENCODER_TEX_THREAD_LOCAL

#define obs (encoder_tex_current->core)
struct obs_core_video_mix *oracle_encoder_tex_mix_walk(video_t *video);
#define get_mix_for_video oracle_encoder_tex_mix_walk
#include "encoder-video-tex-active-mix.inc"
#undef get_mix_for_video
#undef obs

static struct obs_core_video_mix *encoder_tex_lookup(video_t *video)
{
	struct encoder_tex_fixture *fixture = encoder_tex_current;
	fixture->lookups++;
	fixture->lookup_ok = fixture->lookup_ok && video == fixture->media;
	struct obs_core_video_mix *selected = oracle_encoder_tex_mix_walk(video);
	for (size_t i = 0; i < fixture->core->video.mixes.num; i++) {
		struct obs_core_video_mix *mix = fixture->core->video.mixes.array[i];
		if (mix == selected) {
			mix->using_nv12_tex = fixture->flags[i][0];
			mix->using_p010_tex = fixture->flags[i][1];
			break;
		}
	}
	return selected;
}

#define get_mix_for_video encoder_tex_lookup
bool oracle_encoder_video_tex_active(const obs_encoder_t *encoder, enum video_format format);
#define obs_encoder_video_tex_active oracle_encoder_video_tex_active
#include "encoder-video-tex-active.inc"
#undef obs_encoder_video_tex_active
#include "encoder-video-tex-active-fields.inc"
#undef get_mix_for_video

void oracle_encoder_tex_format_values(int *values);
void oracle_encoder_tex_format_values(int *values)
{
	values[0] = VIDEO_FORMAT_NV12;
	values[1] = VIDEO_FORMAT_P010;
	values[2] = VIDEO_FORMAT_R10L;
}

typedef bool (*encoder_tex_query)(const obs_encoder_t *, enum video_format);
bool oracle_encoder_tex_check(encoder_tex_query candidate, int format, uint8_t media, uint8_t mask,
			      const uint8_t *videos, const uint8_t *flags);
bool oracle_encoder_tex_check(encoder_tex_query candidate, int format, uint8_t media, uint8_t mask,
			      const uint8_t *videos, const uint8_t *flags)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	struct obs_core *before = malloc(sizeof(*core));
	struct obs_core *after = malloc(sizeof(*core));
	if (!core || !before || !after) {
		free(core);
		free(before);
		free(after);
		return false;
	}
	if (pthread_mutex_init(&core->video.mixes_mutex, NULL) != 0) {
		free(core);
		free(before);
		free(after);
		return false;
	}
	unsigned char tokens[3] = {11, 22, 33};
	video_t *identities[] = {NULL, (video_t *)&tokens[0], (video_t *)&tokens[1], (video_t *)&tokens[2]};
	struct obs_encoder encoder = {0}, encoder_before;
	struct obs_core_video_mix mixes[3] = {0}, initial[3], c_after[3];
	struct obs_core_video_mix *pointers[3] = {0};
	struct encoder_tex_fixture fixture = {.core = core, .media = identities[media % 4], .lookup_ok = true};
	encoder.media = fixture.media;
	memcpy(&encoder_before, &encoder, sizeof(encoder));
	for (unsigned i = 0; i < 3; i++) {
		mixes[i].video = identities[videos[i] % 4];
		/* Only a matching lookup changes these to the requested values. */
		mixes[i].using_nv12_tex = !(flags[i] & 1);
		mixes[i].using_p010_tex = !(flags[i] & 2);
		if (mask & (1u << i)) {
			size_t slot = core->video.mixes.num++;
			pointers[slot] = &mixes[i];
			fixture.flags[slot][0] = (flags[i] & 1) != 0;
			fixture.flags[slot][1] = (flags[i] & 2) != 0;
		}
	}
	core->video.mixes.array = pointers;
	core->video.mixes.capacity = 3;
	memcpy(initial, mixes, sizeof(mixes));
	memcpy(before, core, sizeof(*core));
	/* Mutex internals may change on any platform; compare all other storage. */
	memset(&before->video.mixes_mutex, 0, sizeof(before->video.mixes_mutex));
	encoder_tex_current = &fixture;
	bool c = oracle_encoder_video_tex_active(&encoder, (enum video_format)format);
	bool equal = fixture.lookups == 1 && fixture.lookup_ok;
	memcpy(c_after, mixes, sizeof(mixes));
	memcpy(mixes, initial, sizeof(mixes));
	fixture.lookups = 0;
	fixture.lookup_ok = true;
	bool rust = candidate(&encoder, (enum video_format)format);
	equal = equal && c == rust && fixture.lookups == 1 && fixture.lookup_ok &&
		memcmp(&encoder, &encoder_before, sizeof(encoder)) == 0 && memcmp(mixes, c_after, sizeof(mixes)) == 0;
	memcpy(after, core, sizeof(*core));
	memset(&after->video.mixes_mutex, 0, sizeof(after->video.mixes_mutex));
	equal = equal && memcmp(before, after, sizeof(*core)) == 0;
	int locked = pthread_mutex_trylock(&core->video.mixes_mutex);
	equal = equal && locked == 0;
	if (locked == 0)
		pthread_mutex_unlock(&core->video.mixes_mutex);
	encoder_tex_current = NULL;
	pthread_mutex_destroy(&core->video.mixes_mutex);
	free(after);
	free(before);
	free(core);
	return equal;
}
