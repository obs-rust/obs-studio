/* Original C getter and real field adapters over thread-local, real-layout
 * fixtures. Excludes an uninitialized core, dead canvases or mixes and a
 * concurrent render or video reset: the original getter requires stable
 * caller state. Texture pointers are opaque tokens and are never
 * dereferenced. The real main canvas and its lifecycle stay unchanged Tier 2
 * (test_get_main_texture, test_main_mix_before_video). There are no
 * intentional behavior differences. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct get_main_texture_fixture {
	struct obs_core core;
	struct obs_canvas canvases[2];
	struct obs_core_video_mix mixes[2];
	unsigned char tokens[2][64];
};

#if defined(_MSC_VER)
static __declspec(thread) struct get_main_texture_fixture *get_main_texture_current;
#else
static _Thread_local struct get_main_texture_fixture *get_main_texture_current;
#endif

gs_texture_t *oracle_obs_get_main_texture(void);
#define obs (&get_main_texture_current->core)
#define obs_get_main_texture oracle_obs_get_main_texture
#include "libobs/obs-get-main-texture.c.h"
#undef obs_get_main_texture
#include "get-main-texture-fields.inc"
#undef obs

void oracle_get_main_texture_layout(size_t *layout);
void oracle_get_main_texture_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_get_main_texture_fields);
	layout[1] = _Alignof(struct obs_get_main_texture_fields);
	layout[2] = offsetof(struct obs_get_main_texture_fields, canvas);
	layout[3] = offsetof(struct obs_get_main_texture_fields, mix);
	layout[4] = offsetof(struct obs_get_main_texture_fields, rendered);
	layout[5] = offsetof(struct obs_get_main_texture_fields, texture);
}

/* One state of the fixture: which canvas is main (or none), which mix that
 * canvas has (or none), whether that mix is rendered and which token is its
 * texture (or none). The other canvas always has the other mix, rendered
 * with a texture, so reading the wrong canvas or mix is visible. */
struct get_main_texture_state {
	int canvas;
	int mix;
	int rendered;
	int texture;
};

static void get_main_texture_apply(struct get_main_texture_fixture *f, struct get_main_texture_state s)
{
	f->core.data.main_canvas = s.canvas < 0 ? NULL : &f->canvases[s.canvas];
	for (int i = 0; i < 2; ++i) {
		f->canvases[i].mix = &f->mixes[1 - i];
		f->mixes[i].render_texture = (gs_texture_t *)f->tokens[1 - i];
		f->mixes[i].texture_rendered = true;
	}
	if (s.canvas >= 0) {
		struct obs_core_video_mix *mix = s.mix < 0 ? NULL : &f->mixes[s.mix];
		f->canvases[s.canvas].mix = mix;
		if (mix) {
			mix->texture_rendered = s.rendered != 0;
			mix->render_texture = s.texture < 0 ? NULL : (gs_texture_t *)f->tokens[s.texture];
		}
	}
}

typedef gs_texture_t *(*get_main_texture_fn)(void);
bool oracle_get_main_texture_check(const int *states, size_t count, unsigned char fill, get_main_texture_fn getter);
bool oracle_get_main_texture_check(const int *states, size_t count, unsigned char fill, get_main_texture_fn getter)
{
	struct get_main_texture_fixture *f = calloc(1, sizeof(*f));
	struct get_main_texture_fixture *before = malloc(sizeof(*f));
	bool valid = f && before;
	if (valid) {
		memset(f->tokens[0], fill, sizeof(f->tokens[0]));
		memset(f->tokens[1], (unsigned char)~fill, sizeof(f->tokens[1]));
		get_main_texture_current = f;
		/* Switch the state between reads: caching an old answer is wrong. */
		for (size_t step = 0; step < count; ++step) {
			const int *v = &states[4 * step];
			struct get_main_texture_state s = {v[0], v[1], v[2], v[3]};
			get_main_texture_apply(f, s);
			memcpy(before, f, sizeof(*f));
			gs_texture_t *expected = oracle_obs_get_main_texture();
			valid = valid && memcmp(before, f, sizeof(*f)) == 0;
			for (unsigned i = 0; i < 8; ++i)
				valid = valid && getter() == expected && memcmp(before, f, sizeof(*f)) == 0;
		}
		get_main_texture_current = NULL;
	}
	free(before);
	free(f);
	return valid;
}

/* What the original answers for one state, for the Rust side to compare its
 * expectations against: -1 for NULL, else the token index. */
int oracle_get_main_texture_expected(int canvas, int mix, int rendered, int texture);
int oracle_get_main_texture_expected(int canvas, int mix, int rendered, int texture)
{
	struct get_main_texture_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return -2;
	get_main_texture_current = f;
	get_main_texture_apply(f, (struct get_main_texture_state){canvas, mix, rendered, texture});
	gs_texture_t *result = oracle_obs_get_main_texture();
	get_main_texture_current = NULL;
	int index = result == NULL                           ? -1
		    : result == (gs_texture_t *)f->tokens[0] ? 0
		    : result == (gs_texture_t *)f->tokens[1] ? 1
							     : -3;
	free(f);
	return index;
}
