/* Original C getter and real field adapter. Exclude uninitialized/dead core and
 * concurrently reset audio: the original getter requires stable caller state.
 * Audio pointers are opaque allocated tokens and are never dereferenced here.
 * Real output lifecycle/activation is covered by unchanged actual-library C tests. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *get_audio_current;
#else
static _Thread_local struct obs_core *get_audio_current;
#endif

audio_t *oracle_obs_get_audio(void);
#define obs get_audio_current
#define obs_get_audio oracle_obs_get_audio
#include "libobs/obs-get-audio.c.h"
#undef obs_get_audio
#include "get-audio-fields.inc"
#undef obs

void oracle_get_audio_layout(size_t *layout);
void oracle_get_audio_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_get_audio_fields);
	layout[1] = _Alignof(struct obs_get_audio_fields);
	layout[2] = offsetof(struct obs_get_audio_fields, audio);
}

typedef audio_t *(*get_audio_fn)(void);
bool oracle_get_audio_check(bool present, bool second, unsigned char fill, get_audio_fn getter);
bool oracle_get_audio_check(bool present, bool second, unsigned char fill, get_audio_fn getter)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	struct obs_core *before = malloc(sizeof(*before));
	unsigned char *tokens[2] = {malloc(64), malloc(64)};
	bool valid = core && before && tokens[0] && tokens[1];
	if (valid) {
		memset(tokens[0], fill, 64);
		memset(tokens[1], (unsigned char)~fill, 64);
		get_audio_current = core;
		/* Switch the current field between reads: caching any old result is wrong. */
		audio_t *sequence[] = {present ? (audio_t *)tokens[second ? 1 : 0] : NULL,
				       (audio_t *)tokens[second ? 0 : 1], NULL, (audio_t *)tokens[second ? 1 : 0]};
		for (size_t step = 0; step < 4; ++step) {
			core->audio.audio = sequence[step];
			memcpy(before, core, sizeof(*core));
			audio_t *expected = oracle_obs_get_audio();
			valid = valid && expected == sequence[step] && memcmp(before, core, sizeof(*core)) == 0;
			for (unsigned i = 0; i < 32; ++i)
				valid = valid && getter() == expected && memcmp(before, core, sizeof(*core)) == 0;
		}
		for (size_t i = 0; i < 64; ++i)
			valid = valid && tokens[0][i] == fill && tokens[1][i] == (unsigned char)~fill;
		get_audio_current = NULL;
	}
	free(tokens[1]);
	free(tokens[0]);
	free(before);
	free(core);
	return valid;
}
