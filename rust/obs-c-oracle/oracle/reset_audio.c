/* Original-C oracle of obs_reset_audio (#5482). The full reset it calls,
 * obs_reset_audio2, is observed, not reimplemented: one recording stand-in
 * receives the original's call and, through the private adapter
 * obs_reset_audio_fields(), the shim's. Its gates, defaults and message are
 * the unchanged Tier 2 (test_reset_audio, test_get_audio). A NULL info is
 * outside the contract (the original reads through it), excluded. */
#include "libobs/build/libobs.pch.h"
#include <string.h>

struct reset_audio_record {
	unsigned calls;
	bool non_null;
	uint32_t samples_per_sec;
	enum speaker_layout speakers;
	uint32_t max_buffering_ms;
	bool fixed_buffering;
	bool result;
};
#if defined(_MSC_VER)
static __declspec(thread) struct reset_audio_record *reset_audio_current;
#else
static _Thread_local struct reset_audio_record *reset_audio_current;
#endif

static bool reset_audio_observe(const struct obs_audio_info2 *oai2)
{
	struct reset_audio_record *r = reset_audio_current;
	r->calls++;
	r->non_null = oai2 != NULL;
	if (oai2) {
		r->samples_per_sec = oai2->samples_per_sec;
		r->speakers = oai2->speakers;
		r->max_buffering_ms = oai2->max_buffering_ms;
		r->fixed_buffering = oai2->fixed_buffering;
	}
	return r->result;
}

bool oracle_obs_reset_audio(const struct obs_audio_info *oai);
#define obs_reset_audio2 reset_audio_observe
#define obs_reset_audio oracle_obs_reset_audio
#include "libobs/obs-reset-audio.c.h"
#undef obs_reset_audio
#include "reset-audio-fields.inc"
#undef obs_reset_audio2

typedef bool (*reset_audio_fn)(const struct obs_audio_info *);

/* Runs the original and `shim` on the same info against the same stand-in
 * answer; true when both made exactly one call with the same fields, the
 * shim's call carried the two fields taken with the rest zeroed, and both
 * returned the stand-in's answer. */
bool oracle_reset_audio_check(uint32_t samples_per_sec, int32_t speakers, bool result, reset_audio_fn shim);
bool oracle_reset_audio_check(uint32_t samples_per_sec, int32_t speakers, bool result, reset_audio_fn shim)
{
	const struct obs_audio_info info = {
		.samples_per_sec = samples_per_sec,
		.speakers = (enum speaker_layout)speakers,
	};
	struct reset_audio_record c_record;
	struct reset_audio_record rust_record;

	memset(&c_record, 0xA5, sizeof(c_record));
	c_record.calls = 0;
	c_record.result = result;
	reset_audio_current = &c_record;
	bool c_result = oracle_obs_reset_audio(&info);

	memset(&rust_record, 0x5A, sizeof(rust_record));
	rust_record.calls = 0;
	rust_record.result = result;
	reset_audio_current = &rust_record;
	bool rust_result = shim(&info);
	reset_audio_current = NULL;

	return c_record.calls == 1 && rust_record.calls == 1 && c_record.non_null && rust_record.non_null &&
	       c_result == result && rust_result == result && c_record.samples_per_sec == samples_per_sec &&
	       c_record.speakers == info.speakers && c_record.max_buffering_ms == 0 && !c_record.fixed_buffering &&
	       rust_record.samples_per_sec == c_record.samples_per_sec && rust_record.speakers == c_record.speakers &&
	       rust_record.max_buffering_ms == c_record.max_buffering_ms &&
	       rust_record.fixed_buffering == c_record.fixed_buffering;
}
