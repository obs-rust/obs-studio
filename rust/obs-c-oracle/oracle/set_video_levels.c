/* Original obs_set_video_levels() and the real-header adapters of
 * obs-set-video-levels-fields.c.h on one recording fixture core. Under cargo
 * test these adapters are the only definitions of their real names, so the
 * Rust shim reaches this fixture. blog is recorded for both (level, format
 * string and the bits of each %g argument as passed), so a match means the
 * same text byte for byte; the probe runs a script of calls on the original
 * and on a candidate from the same core and reports, instead of aborting,
 * any difference in the levels after each call, the core bytes or the log,
 * so mutants are observable.
 *
 * The original's level rules are the Rust obs_core_setup_rules_levels_core
 * and obs_core_setup_rules_level_ok, as in libobs. With no core the original
 * warns and returns (#3397). Excluded: a core without a graphics subsystem
 * (the original asserts; the shim alone is checked to touch nothing), an
 * uninitialized or dead core. No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define SVL_RECORDS 64
#define SVL_CALLS 64

struct svl_record {
	int level;
	const char *format;
	unsigned args;
	uint64_t bits[2];
};

struct svl_log {
	size_t count;
	struct svl_record records[SVL_RECORDS];
};

static _Thread_local struct obs_core svl_core;
static _Thread_local struct obs_core *svl_current;
static _Thread_local struct svl_log svl_log;
/* A stand-in graphics subsystem, only compared with NULL. */
static char svl_graphics;

static struct obs_core *svl_read_core(void)
{
	return svl_current;
}

static unsigned svl_count_args(const char *format)
{
	unsigned n = 0;
	for (const char *p = format; (p = strstr(p, "%g")) != NULL; p += 2)
		n++;
	return n;
}

static void svl_blog(int level, const char *format, ...)
{
	if (svl_log.count >= SVL_RECORDS) {
		svl_log.count++;
		return;
	}
	struct svl_record *r = &svl_log.records[svl_log.count++];
	memset(r, 0, sizeof(*r));
	r->level = level;
	r->format = format;
	r->args = svl_count_args(format);
	va_list args;
	va_start(args, format);
	for (unsigned i = 0; i < r->args && i < 2; i++) {
		double value = va_arg(args, double);
		memcpy(&r->bits[i], &value, sizeof(value));
	}
	va_end(args);
}

void oracle_obs_set_video_levels(float sdr_white_level, float hdr_nominal_peak_level);

#define obs svl_read_core()
#define blog svl_blog
#define obs_set_video_levels oracle_obs_set_video_levels
#include "libobs/obs-set-video-levels.c.h"
#undef obs_set_video_levels
#include "set_video_levels_fields.inc"
#undef blog
#undef obs

typedef void (*svl_fn)(float, float);

/* The levels after each call, as bits. */
struct svl_run {
	uint32_t levels[SVL_CALLS][2];
	struct svl_log log;
};

static void svl_arm(bool core, bool graphics, float sdr, float hdr)
{
	memset(&svl_core, 0, sizeof(svl_core));
	svl_core.video.graphics = graphics ? (graphics_t *)(void *)&svl_graphics : NULL;
	svl_core.video.sdr_white_level = sdr;
	svl_core.video.hdr_nominal_peak_level = hdr;
	svl_current = core ? &svl_core : NULL;
	memset(&svl_log, 0, sizeof(svl_log));
}

static void svl_disarm(void)
{
	svl_current = NULL;
	memset(&svl_core, 0, sizeof(svl_core));
	memset(&svl_log, 0, sizeof(svl_log));
}

/* `calls` holds `n` (white, peak) pairs. With no core the levels read are
 * the fixture's, which the calls must not change. */
static void svl_run(svl_fn fn, bool core, float sdr, float hdr, const float *calls, size_t n, struct svl_run *out,
		    struct obs_core *after)
{
	svl_arm(core, true, sdr, hdr);
	memset(out, 0, sizeof(*out));
	for (size_t i = 0; i < n; i++) {
		fn(calls[2 * i], calls[2 * i + 1]);
		memcpy(&out->levels[i][0], &svl_core.video.sdr_white_level, sizeof(float));
		memcpy(&out->levels[i][1], &svl_core.video.hdr_nominal_peak_level, sizeof(float));
	}
	out->log = svl_log;
	memcpy(after, &svl_core, sizeof(*after));
	svl_disarm();
}

static bool svl_same_log(const struct svl_log *a, const struct svl_log *b)
{
	if (a->count != b->count)
		return false;
	for (size_t i = 0; i < a->count && i < SVL_RECORDS; i++) {
		const struct svl_record *x = &a->records[i], *y = &b->records[i];
		if (x->level != y->level || strcmp(x->format, y->format) != 0 || x->args != y->args ||
		    memcmp(x->bits, y->bits, sizeof(x->bits)) != 0)
			return false;
	}
	return true;
}

/* 0: match. 1: the levels after some call differ, 2: the logs differ, 4: the
 * core bytes differ at the end, 8: with no core, a call changed the
 * fixture. 0x100: too many calls. */
unsigned oracle_set_video_levels_probe(svl_fn candidate, bool core, float sdr, float hdr, const float *calls, size_t n);
unsigned oracle_set_video_levels_probe(svl_fn candidate, bool core, float sdr, float hdr, const float *calls, size_t n)
{
	static _Thread_local struct svl_run original, actual;
	static _Thread_local struct obs_core original_after, actual_after, armed;
	if (n > SVL_CALLS)
		return 0x100;
	svl_run(oracle_obs_set_video_levels, core, sdr, hdr, calls, n, &original, &original_after);
	svl_run(candidate, core, sdr, hdr, calls, n, &actual, &actual_after);
	unsigned diff = 0;
	if (memcmp(original.levels, actual.levels, sizeof(original.levels)) != 0)
		diff |= 1;
	if (!svl_same_log(&original.log, &actual.log))
		diff |= 2;
	if (memcmp(&original_after, &actual_after, sizeof(original_after)) != 0)
		diff |= 4;
	if (!core) {
		svl_arm(false, true, sdr, hdr);
		memcpy(&armed, &svl_core, sizeof(armed));
		svl_disarm();
		if (memcmp(&armed, &original_after, sizeof(armed)) != 0 ||
		    memcmp(&armed, &actual_after, sizeof(armed)) != 0)
			diff |= 8;
	}
	return diff;
}

/* The original's log of one call from (sdr, hdr): up to `max` records of the
 * level, the format and the argument bits, for the characterization checks.
 * Returns the number of records, or -1 for too many. */
int oracle_set_video_levels_original_log(bool core, float sdr, float hdr, float white, float peak, int *levels,
					 const char **formats, uint64_t *bits, int max);
int oracle_set_video_levels_original_log(bool core, float sdr, float hdr, float white, float peak, int *levels,
					 const char **formats, uint64_t *bits, int max)
{
	svl_arm(core, true, sdr, hdr);
	oracle_obs_set_video_levels(white, peak);
	struct svl_log log = svl_log;
	svl_disarm();
	if (log.count > (size_t)max || log.count > SVL_RECORDS)
		return -1;
	for (size_t i = 0; i < log.count; i++) {
		levels[i] = log.records[i].level;
		formats[i] = log.records[i].format;
		bits[2 * i] = log.records[i].bits[0];
		bits[2 * i + 1] = log.records[i].bits[1];
	}
	return (int)log.count;
}

/* The candidate alone on a core without graphics: true when no call
 * changed the core or logged anything (the original asserts there). */
bool oracle_set_video_levels_no_graphics(svl_fn candidate, float white, float peak);
bool oracle_set_video_levels_no_graphics(svl_fn candidate, float white, float peak)
{
	static _Thread_local struct obs_core before;
	svl_arm(true, false, 300.0f, 1000.0f);
	memcpy(&before, &svl_core, sizeof(before));
	candidate(white, peak);
	bool ok = memcmp(&before, &svl_core, sizeof(before)) == 0 && svl_log.count == 0;
	svl_disarm();
	return ok;
}

/* The adapters against the real fields and the recorder. */
bool oracle_set_video_levels_adapters(void);
bool oracle_set_video_levels_adapters(void)
{
	svl_arm(false, false, 0.0f, 0.0f);
	bool ok = !obs_set_video_levels_has_core();
	obs_set_video_levels_warn_no_core();
	ok = ok && svl_log.count == 1 && svl_log.records[0].level == LOG_WARNING &&
	     strcmp(svl_log.records[0].format, "obs_set_video_levels: no core, ignoring the levels") == 0 &&
	     svl_log.records[0].args == 0;

	svl_arm(true, false, 300.0f, 1000.0f);
	ok = ok && obs_set_video_levels_has_core() && !obs_set_video_levels_has_graphics();
	svl_core.video.graphics = (graphics_t *)(void *)&svl_graphics;
	ok = ok && obs_set_video_levels_has_graphics();
	ok = ok && obs_set_video_levels_sdr_white_level() == 300.0f &&
	     obs_set_video_levels_hdr_nominal_peak_level() == 1000.0f;
	obs_set_video_levels_store_sdr_white_level(123.0f);
	ok = ok && svl_core.video.sdr_white_level == 123.0f && svl_core.video.hdr_nominal_peak_level == 1000.0f;
	obs_set_video_levels_store_hdr_nominal_peak_level(456.0f);
	ok = ok && svl_core.video.sdr_white_level == 123.0f && svl_core.video.hdr_nominal_peak_level == 456.0f;

	double sdr_args[2] = {-1.0, 200.0}, hdr_args[2] = {0.0, 700.0};
	obs_set_video_levels_warn_sdr_white_level(-1.0f, 200.0f);
	obs_set_video_levels_warn_hdr_nominal_peak_level(0.0f, 700.0f);
	ok = ok && svl_log.count == 2 && svl_log.records[0].level == LOG_WARNING &&
	     strcmp(svl_log.records[0].format, "obs_set_video_levels: ignoring SDR white level %g, keeping %g") == 0 &&
	     memcmp(svl_log.records[0].bits, sdr_args, sizeof(sdr_args)) == 0 &&
	     svl_log.records[1].level == LOG_WARNING &&
	     strcmp(svl_log.records[1].format,
		    "obs_set_video_levels: ignoring HDR nominal peak level %g, keeping %g") == 0 &&
	     memcmp(svl_log.records[1].bits, hdr_args, sizeof(hdr_args)) == 0;
	svl_disarm();
	return ok;
}
