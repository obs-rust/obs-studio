/* Original obs_get_audio_info() and obs_get_audio_info2() (#5363), renamed,
 * beside the real adapters of obs-audio-info-queries-fields.c.h. Both read a
 * thread-local current core instead of the library's global, and both reach
 * audio_output_get_info() through one recording probe, so the original and
 * the Rust shim see the same core and the same output info. The milliseconds
 * helper is the real obs_core_setup_rules_buffer_ms() for both. The probe
 * reports, instead of aborting, any difference in the result, the caller's
 * output bytes (sentinel-filled), the info call log, the core bytes or the
 * current core pointer, so mutants are observable. Excludes a dead core and
 * concurrent writes to the audio fields: the original requires stable caller
 * state. Real cores stay covered by the unchanged actual-library C tests. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *audio_info_current;
static __declspec(thread) struct audio_output_info audio_info_output;
static __declspec(thread) char audio_info_log[16];
static __declspec(thread) size_t audio_info_log_len;
#else
static _Thread_local struct obs_core *audio_info_current;
static _Thread_local struct audio_output_info audio_info_output;
static _Thread_local char audio_info_log[16];
static _Thread_local size_t audio_info_log_len;
#endif

/* A non-NULL handle the probe compares against; never dereferenced. */
static char audio_info_fake_storage;
#define AUDIO_INFO_FAKE ((audio_t *)&audio_info_fake_storage)

static const struct audio_output_info *audio_info_probe_get_info(const audio_t *audio)
{
	/* 'i' for the expected handle, 'x' for any other. */
	char entry = audio == AUDIO_INFO_FAKE ? 'i' : 'x';
	if (audio_info_log_len < sizeof(audio_info_log))
		audio_info_log[audio_info_log_len++] = entry;
	return &audio_info_output;
}

bool oracle_obs_get_audio_info(struct obs_audio_info *oai);
bool oracle_obs_get_audio_info2(struct obs_audio_info2 *oai2);
#define obs audio_info_current
#define audio_output_get_info audio_info_probe_get_info
#define obs_get_audio_info oracle_obs_get_audio_info
#define obs_get_audio_info2 oracle_obs_get_audio_info2
#include "libobs/obs-audio-info-queries.c.h"
#undef obs_get_audio_info2
#undef obs_get_audio_info
#include "audio_info_queries_fields.inc"
#undef audio_output_get_info
#undef obs

/* sizeof/alignof and field offsets of obs_audio_info, obs_audio_info2 and
 * the two audio_output_info fields read. */
void oracle_audio_info_queries_layout(size_t *out);
void oracle_audio_info_queries_layout(size_t *out)
{
	out[0] = sizeof(struct obs_audio_info);
	out[1] = _Alignof(struct obs_audio_info);
	out[2] = offsetof(struct obs_audio_info, samples_per_sec);
	out[3] = offsetof(struct obs_audio_info, speakers);
	out[4] = sizeof(struct obs_audio_info2);
	out[5] = _Alignof(struct obs_audio_info2);
	out[6] = offsetof(struct obs_audio_info2, samples_per_sec);
	out[7] = offsetof(struct obs_audio_info2, speakers);
	out[8] = offsetof(struct obs_audio_info2, max_buffering_ms);
	out[9] = offsetof(struct obs_audio_info2, fixed_buffering);
	out[10] = offsetof(struct audio_output_info, samples_per_sec);
	out[11] = offsetof(struct audio_output_info, speakers);
	out[12] = sizeof(enum speaker_layout);
}

typedef bool (*audio_info_fn)(struct obs_audio_info *oai);
typedef bool (*audio_info2_fn)(struct obs_audio_info2 *oai2);

struct audio_info_run {
	bool result;
	union {
		struct obs_audio_info info;
		struct obs_audio_info2 info2;
	} out;
	char log[16];
	size_t log_len;
};

/* which: 1 = obs_get_audio_info, 2 = obs_get_audio_info2. */
static bool audio_info_run(int which, audio_info_fn fn1, audio_info2_fn fn2, bool out_null, uint8_t fill,
			   struct audio_info_run *run)
{
	memset(run, 0, sizeof(*run));
	memset(&run->out, fill, sizeof(run->out));
	audio_info_log_len = 0;
	if (which == 1)
		run->result = fn1(out_null ? NULL : &run->out.info);
	else
		run->result = fn2(out_null ? NULL : &run->out.info2);
	memcpy(run->log, audio_info_log, sizeof(run->log));
	run->log_len = audio_info_log_len;
	return true;
}

/* mode: 0 = NULL core, 1 = core without an audio output, 2 = with one.
 * 0: match. 1: results differ, 2: output bytes differ, 4: info call logs
 * differ, 8: the candidate changed the core, 16: it changed the current
 * core pointer, 32: allocation failed. *original receives the original's
 * result. */
unsigned oracle_audio_info_queries_probe(int which, audio_info_fn cand1, audio_info2_fn cand2, int mode, bool out_null,
					 uint8_t fill, uint32_t rate, int speakers, bool fixed, int ticks,
					 bool *original);
unsigned oracle_audio_info_queries_probe(int which, audio_info_fn cand1, audio_info2_fn cand2, int mode, bool out_null,
					 uint8_t fill, uint32_t rate, int speakers, bool fixed, int ticks,
					 bool *original)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	struct obs_core *before = malloc(sizeof(*before));
	unsigned diff = 0;
	if (!core || !before) {
		diff = 32;
		goto done;
	}
	memset(core, fill, sizeof(*core));
	core->audio.audio = mode == 2 ? AUDIO_INFO_FAKE : NULL;
	core->audio.fixed_buffer = fixed;
	core->audio.max_buffering_ticks = ticks;
	memset(&audio_info_output, fill, sizeof(audio_info_output));
	audio_info_output.samples_per_sec = rate;
	audio_info_output.speakers = (enum speaker_layout)speakers;
	struct obs_core *owner = mode ? core : NULL;
	audio_info_current = owner;
	memcpy(before, core, sizeof(*core));

	struct audio_info_run want, got;
	audio_info_run(which, oracle_obs_get_audio_info, oracle_obs_get_audio_info2, out_null, fill, &want);
	audio_info_run(which, cand1, cand2, out_null, fill, &got);
	*original = want.result;

	if (want.result != got.result)
		diff |= 1;
	if (memcmp(&want.out, &got.out, sizeof(want.out)) != 0)
		diff |= 2;
	if (want.log_len != got.log_len || memcmp(want.log, got.log, sizeof(want.log)) != 0)
		diff |= 4;
	if (memcmp(before, core, sizeof(*core)) != 0)
		diff |= 8;
	if (audio_info_current != owner)
		diff |= 16;
	audio_info_current = NULL;

done:
	free(before);
	free(core);
	return diff;
}
