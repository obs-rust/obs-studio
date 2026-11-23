/* Original obs_reset_audio2() (#5486) and the real adapters of
 * obs-reset-audio2-fields.c.h on one recording fixture core. Under cargo
 * test these adapters are the only definitions of their real names, so the
 * Rust shim reaches this fixture. Both sides reach the same recording
 * stand-ins: audio_output_active() (the handle it is asked about and the
 * answer set), obs_free_audio() (clears the output and the two buffering
 * fields, as its memset does), obs_init_audio() (the info passed: name,
 * rate, format, layout and callback, and the two buffering fields as they
 * are when it is called; opens the stand-in output on success) and blog()
 * (level, format text, each %d and the %s text, so a match is the same
 * logged text byte for byte). The probe runs the original and a candidate
 * from the same core and reports, instead of aborting, any difference in
 * the result, the step log or the core's audio fields afterwards, so
 * mutants are observable.
 *
 * The original's rules are the Rust obs_core_setup_rules_*, as in libobs.
 * Excluded: a dead or uninitialized core and an unreadable info (the
 * original reads both unchecked). The real obs_init_audio/obs_free_audio
 * are the unchanged Tier 2 (test_reset_audio2, test_get_audio). No
 * intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define RA2_EVENTS 16
#define RA2_TEXT 192

struct ra2_event {
	/* 'a' audio_output_active, 'f' obs_free_audio, 'i' obs_init_audio,
	 * 'b' blog. */
	char kind;
	int level;
	/* blog: its %d arguments; obs_init_audio: rate, format, layout, the
	 * callback is the real one's stand-in (1), the stored ticks and fixed
	 * flag; audio_output_active: the handle is the core's output (1). */
	int ints[6];
	unsigned nints;
	/* blog: the format; obs_init_audio: the name. */
	char text[RA2_TEXT];
	/* blog: the %s argument. */
	char arg[32];
};

struct ra2_log {
	size_t count;
	struct ra2_event events[RA2_EVENTS];
};

static _Thread_local struct obs_core ra2_core;
static _Thread_local struct obs_core *ra2_current;
static _Thread_local struct ra2_log ra2_log;
static _Thread_local bool ra2_active;
static _Thread_local bool ra2_init_answer;
/* A non-NULL output handle, only compared. */
static char ra2_output_storage;
#define RA2_OUTPUT ((audio_t *)&ra2_output_storage)

static struct obs_core *ra2_read_core(void)
{
	return ra2_current;
}

static void ra2_copy(char *dst, size_t size, const char *src)
{
	size_t i = 0;
	if (src)
		for (; i + 1 < size && src[i]; i++)
			dst[i] = src[i];
	dst[i] = '\0';
}

static struct ra2_event *ra2_push(char kind)
{
	static struct ra2_event overflow;
	if (ra2_log.count >= RA2_EVENTS) {
		ra2_log.count++;
		return &overflow;
	}
	struct ra2_event *e = &ra2_log.events[ra2_log.count++];
	memset(e, 0, sizeof(*e));
	e->kind = kind;
	return e;
}

static bool ra2_output_active(const audio_t *audio)
{
	struct ra2_event *e = ra2_push('a');
	e->ints[0] = audio == RA2_OUTPUT;
	e->nints = 1;
	return ra2_active;
}

static bool ra2_callback(void *param, uint64_t start_ts, uint64_t end_ts, uint64_t *new_ts, uint32_t active_mixers,
			 struct audio_output_data *mixes)
{
	(void)param;
	(void)start_ts;
	(void)end_ts;
	(void)new_ts;
	(void)active_mixers;
	(void)mixes;
	return false;
}

static void ra2_free_audio(void)
{
	ra2_push('f');
	ra2_core.audio.audio = NULL;
	ra2_core.audio.max_buffering_ticks = 0;
	ra2_core.audio.fixed_buffer = false;
}

static bool ra2_init_audio(struct audio_output_info *ai)
{
	struct ra2_event *e = ra2_push('i');
	ra2_copy(e->text, sizeof(e->text), ai->name);
	e->ints[0] = (int)ai->samples_per_sec;
	e->ints[1] = (int)ai->format;
	e->ints[2] = (int)ai->speakers;
	e->ints[3] = ai->input_callback == ra2_callback;
	e->ints[4] = ra2_core.audio.max_buffering_ticks;
	e->ints[5] = ra2_core.audio.fixed_buffer;
	e->nints = 6;
	if (ra2_init_answer)
		ra2_core.audio.audio = RA2_OUTPUT;
	return ra2_init_answer;
}

void oracle_reset_audio2_blog(int level, const char *format, ...);
void oracle_reset_audio2_blog(int level, const char *format, ...)
{
	struct ra2_event *e = ra2_push('b');
	e->level = level;
	ra2_copy(e->text, sizeof(e->text), format);
	va_list args;
	va_start(args, format);
	for (const char *p = format; p && *p; p++) {
		if (*p != '%')
			continue;
		p++;
		if (*p == 'd') {
			int value = va_arg(args, int);
			if (e->nints < 6)
				e->ints[e->nints++] = value;
		} else if (*p == 's') {
			ra2_copy(e->arg, sizeof(e->arg), va_arg(args, const char *));
		} else {
			break;
		}
	}
	va_end(args);
}

bool oracle_obs_reset_audio2(const struct obs_audio_info2 *oai);

#define obs ra2_read_core()
#define blog oracle_reset_audio2_blog
#define audio_output_active ra2_output_active
#define obs_free_audio ra2_free_audio
#define obs_init_audio ra2_init_audio
#define audio_callback ra2_callback
#define obs_reset_audio2 oracle_obs_reset_audio2
#include "libobs/obs-reset-audio2.c.h"
#undef obs_reset_audio2
#include "reset-audio2-fields.inc"
#undef audio_callback
#undef obs_init_audio
#undef obs_free_audio
#undef audio_output_active
#undef blog
#undef obs

typedef bool (*ra2_fn)(const struct obs_audio_info2 *);

struct ra2_setup {
	bool core;
	bool output;
	bool active;
	bool info;
	uint32_t samples_per_sec;
	int speakers;
	uint32_t max_buffering_ms;
	bool fixed_buffering;
	bool init_answer;
	int ticks;
	bool fixed_buffer;
};

struct ra2_run {
	bool result;
	struct ra2_log log;
	bool output;
	bool foreign_output;
	int ticks;
	bool fixed_buffer;
	bool current;
};

static void ra2_run(ra2_fn fn, const struct ra2_setup *s, struct ra2_run *run)
{
	memset(&ra2_core, 0, sizeof(ra2_core));
	memset(&ra2_log, 0, sizeof(ra2_log));
	ra2_core.audio.audio = s->output ? RA2_OUTPUT : NULL;
	ra2_core.audio.max_buffering_ticks = s->ticks;
	ra2_core.audio.fixed_buffer = s->fixed_buffer;
	ra2_current = s->core ? &ra2_core : NULL;
	ra2_active = s->active;
	ra2_init_answer = s->init_answer;

	const struct obs_audio_info2 info = {
		.samples_per_sec = s->samples_per_sec,
		.speakers = (enum speaker_layout)s->speakers,
		.max_buffering_ms = s->max_buffering_ms,
		.fixed_buffering = s->fixed_buffering,
	};

	memset(run, 0, sizeof(*run));
	run->result = fn(s->info ? &info : NULL);
	memcpy(&run->log, &ra2_log, sizeof(ra2_log));
	run->output = ra2_core.audio.audio == RA2_OUTPUT;
	run->foreign_output = ra2_core.audio.audio != NULL && ra2_core.audio.audio != RA2_OUTPUT;
	run->ticks = ra2_core.audio.max_buffering_ticks;
	run->fixed_buffer = ra2_core.audio.fixed_buffer;
	run->current = ra2_current == (s->core ? &ra2_core : NULL);
	ra2_current = NULL;
}

/* 0: the candidate matches the original. Else 1: results differ, 2: step
 * logs differ (count, order, kinds, levels, texts, arguments), 4: the
 * core's audio fields differ afterwards, 8: a log overflowed. *original
 * receives the original's result and *steps its step count. */
unsigned oracle_reset_audio2_probe(ra2_fn candidate, bool core, bool output, bool active, bool info,
				   uint32_t samples_per_sec, int32_t speakers, uint32_t max_buffering_ms,
				   bool fixed_buffering, bool init_answer, int32_t ticks, bool fixed_buffer,
				   bool *original, uint32_t *steps);
unsigned oracle_reset_audio2_probe(ra2_fn candidate, bool core, bool output, bool active, bool info,
				   uint32_t samples_per_sec, int32_t speakers, uint32_t max_buffering_ms,
				   bool fixed_buffering, bool init_answer, int32_t ticks, bool fixed_buffer,
				   bool *original, uint32_t *steps)
{
	const struct ra2_setup s = {
		.core = core,
		.output = output,
		.active = active,
		.info = info,
		.samples_per_sec = samples_per_sec,
		.speakers = speakers,
		.max_buffering_ms = max_buffering_ms,
		.fixed_buffering = fixed_buffering,
		.init_answer = init_answer,
		.ticks = ticks,
		.fixed_buffer = fixed_buffer,
	};
	struct ra2_run c_run;
	struct ra2_run rust_run;
	unsigned diff = 0;

	ra2_run(oracle_obs_reset_audio2, &s, &c_run);
	ra2_run(candidate, &s, &rust_run);

	*original = c_run.result;
	*steps = (uint32_t)c_run.log.count;
	if (c_run.result != rust_run.result)
		diff |= 1;
	if (c_run.log.count != rust_run.log.count ||
	    memcmp(c_run.log.events, rust_run.log.events,
		   sizeof(struct ra2_event) * (c_run.log.count < RA2_EVENTS ? c_run.log.count : RA2_EVENTS)) != 0)
		diff |= 2;
	if (c_run.output != rust_run.output || c_run.foreign_output != rust_run.foreign_output ||
	    c_run.ticks != rust_run.ticks || c_run.fixed_buffer != rust_run.fixed_buffer || !rust_run.current)
		diff |= 4;
	if (c_run.log.count > RA2_EVENTS || rust_run.log.count > RA2_EVENTS)
		diff |= 8;
	return diff;
}
