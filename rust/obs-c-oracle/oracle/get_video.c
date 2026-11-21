/* Original C getter and real field adapters over thread-local, real-layout
 * fixtures. Excludes an uninitialized core, dead canvases or mixes and a
 * concurrent video reset: the original getter requires stable caller state.
 * Video pointers are opaque tokens and are never dereferenced. The real
 * main canvas and its lifecycle stay unchanged Tier 2 (test_get_video).
 * There are no intentional behavior differences. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct get_video_fixture {
	struct obs_core core;
	struct obs_canvas canvases[2];
	struct obs_core_video_mix mixes[2];
	unsigned char tokens[2][64];
};

#if defined(_MSC_VER)
static __declspec(thread) struct get_video_fixture *get_video_current;
#else
static _Thread_local struct get_video_fixture *get_video_current;
#endif

video_t *oracle_obs_get_video(void);
#define obs (&get_video_current->core)
#define obs_get_video oracle_obs_get_video
#include "libobs/obs-get-video.c.h"
#undef obs_get_video
#include "get-video-fields.inc"
#undef obs

void oracle_get_video_layout(size_t *layout);
void oracle_get_video_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_get_video_fields);
	layout[1] = _Alignof(struct obs_get_video_fields);
	layout[2] = offsetof(struct obs_get_video_fields, canvas);
	layout[3] = offsetof(struct obs_get_video_fields, mix);
	layout[4] = offsetof(struct obs_get_video_fields, video);
}

/* One state of the fixture: which canvas is main (or none), which mix that
 * canvas has (or none) and which token that mix's video is (or none). The
 * other canvas always has the other mix with a video, so reading the wrong
 * canvas or mix is visible. */
struct get_video_state {
	int canvas;
	int mix;
	int video;
};

static void get_video_apply(struct get_video_fixture *f, struct get_video_state s)
{
	f->core.data.main_canvas = s.canvas < 0 ? NULL : &f->canvases[s.canvas];
	for (int i = 0; i < 2; ++i) {
		f->canvases[i].mix = &f->mixes[1 - i];
		f->mixes[i].video = (video_t *)f->tokens[1 - i];
	}
	if (s.canvas >= 0) {
		struct obs_core_video_mix *mix = s.mix < 0 ? NULL : &f->mixes[s.mix];
		f->canvases[s.canvas].mix = mix;
		if (mix)
			mix->video = s.video < 0 ? NULL : (video_t *)f->tokens[s.video];
	}
}

typedef video_t *(*get_video_fn)(void);
bool oracle_get_video_check(const int *states, size_t count, unsigned char fill, get_video_fn getter);
bool oracle_get_video_check(const int *states, size_t count, unsigned char fill, get_video_fn getter)
{
	struct get_video_fixture *f = calloc(1, sizeof(*f));
	struct get_video_fixture *before = malloc(sizeof(*f));
	bool valid = f && before;
	if (valid) {
		memset(f->tokens[0], fill, sizeof(f->tokens[0]));
		memset(f->tokens[1], (unsigned char)~fill, sizeof(f->tokens[1]));
		get_video_current = f;
		/* Switch the state between reads: caching an old answer is wrong. */
		for (size_t step = 0; step < count; ++step) {
			struct get_video_state s = {states[3 * step], states[3 * step + 1], states[3 * step + 2]};
			get_video_apply(f, s);
			memcpy(before, f, sizeof(*f));
			video_t *expected = oracle_obs_get_video();
			valid = valid && memcmp(before, f, sizeof(*f)) == 0;
			for (unsigned i = 0; i < 8; ++i)
				valid = valid && getter() == expected && memcmp(before, f, sizeof(*f)) == 0;
		}
		get_video_current = NULL;
	}
	free(before);
	free(f);
	return valid;
}

/* What the original answers for one state, for the Rust side to compare its
 * expectations against: -1 for NULL, else the token index. */
int oracle_get_video_expected(int canvas, int mix, int video);
int oracle_get_video_expected(int canvas, int mix, int video)
{
	struct get_video_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return -2;
	get_video_current = f;
	get_video_apply(f, (struct get_video_state){canvas, mix, video});
	video_t *result = oracle_obs_get_video();
	get_video_current = NULL;
	int index = result == NULL                      ? -1
		    : result == (video_t *)f->tokens[0] ? 0
		    : result == (video_t *)f->tokens[1] ? 1
							: -3;
	free(f);
	return index;
}
