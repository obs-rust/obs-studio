/* Tier 3: the unchanged extracted obs_source_addref(), obs_source_release(),
 * obs_source_get_ref() and obs_weak_source_get_source()
 * (libobs/obs-source-refs.c.h), renamed oracle_*, and the production
 * adapters the Rust shims call (obs-source-refs-fields.c.h), over the real
 * private layout.
 *
 * This file holds the one definition, for the test binaries, of each
 * adapter. obs_weak_source_refs_field() and obs_weak_source_source_field()
 * are always the real ones. The other three dispatch: with a fixture of this
 * file installed on the calling thread, the control and the core check are
 * the real ones (the core check reads the fixture's core, as the original
 * reads `obs`), and the destroy is recorded instead of run. Without one, they
 * serve oracle/view.c's sources, which the view parity test passes to the
 * real obs_source_get_ref() and obs_source_release(): the control stand-in
 * traces the step through oracle_view_ref_step() and hands back a scratch
 * control that refuses a dying source and never reaches a destroy, as the
 * view doubles that stood in for the two entries did. The core check, which
 * only release makes, marks the next control as a release's. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
/* Renamed before the PCH, so obs.h and obs-internal.h declare the renamed
 * originals too. */
#define obs_source_addref oracle_obs_source_addref
#define obs_source_release oracle_obs_source_release
#define obs_source_get_ref oracle_obs_source_get_ref
#define obs_weak_source_get_source oracle_obs_weak_source_get_source
#include "libobs/build/libobs.pch.h"

struct srf_fixture;
static struct obs_core *srf_core(void);
static void srf_record_destroy(obs_source_t *source);

#define obs srf_core()
#define obs_source_destroy srf_record_destroy
#include "source-refs.inc"
#undef obs_weak_source_get_source
#undef obs_source_get_ref
#undef obs_source_release
#undef obs_source_addref

#define obs_source_refs_control srf_real_control
#define obs_source_refs_core_alive srf_real_core_alive
#define obs_source_refs_destroy srf_real_destroy
#include "source_refs_fields.inc"
#undef obs_source_refs_destroy
#undef obs_source_refs_core_alive
#undef obs_source_refs_control
#undef obs_source_destroy
#undef obs

/* One call: addref (0), release (1), get_ref (2) or weak get_source (3) of
 * the fixture's source or control, or of NULL, with the core up or shut
 * down, from the given counts, on a source and control whose every other
 * byte is `noise`. The control may be missing (get_ref only: the others
 * would dereference it) and may name no source. */
struct srf_case {
	uint8_t call;
	bool null_input;
	bool core_alive;
	bool no_control;
	bool no_source;
	uint8_t noise;
	long refs;
	long weak_refs;
};

/* What one side did: what it returned (0 NULL, 1 the source, 2 anything
 * else), the counts afterwards, each destroy (how many, of which source, and
 * the counts at the first), its log lines (count, level of the last, the
 * last), and whether every other byte is as before. */
struct srf_result {
	uint32_t returned;
	long refs;
	long weak_refs;
	uint32_t destroys;
	bool destroyed_source;
	long refs_at_destroy;
	long weak_refs_at_destroy;
	uint32_t log_count;
	int32_t log_level;
	char log[128];
	bool untouched;
};

struct srf_fixture {
	struct obs_core core;
	struct obs_source source;
	struct obs_weak_source control;
	const struct srf_case *c;
	struct srf_result *out;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct srf_fixture *srf_current;

static struct obs_core *srf_core(void)
{
	struct srf_fixture *f = srf_current;
	return f && f->c->core_alive ? &f->core : NULL;
}

static void srf_record_destroy(obs_source_t *source)
{
	struct srf_fixture *f = srf_current;
	if (!f)
		return;
	struct srf_result *out = f->out;
	if (out->destroys++ == 0) {
		out->destroyed_source = source == &f->source;
		out->refs_at_destroy = f->control.ref.refs;
		out->weak_refs_at_destroy = f->control.ref.weak_refs;
	}
}

/* ------------------------------------------------------------------------- */
/* the one definition of each dispatching adapter */

bool oracle_view_ref_step(const obs_source_t *source, bool release);

/* Set by the core check, which only release makes, for the next control. */
static _Thread_local bool srf_view_release;

obs_weak_source_t *obs_source_refs_control(obs_source_t *source);
obs_weak_source_t *obs_source_refs_control(obs_source_t *source)
{
	if (srf_current)
		return srf_real_control(source);

	static _Thread_local struct obs_weak_source scratch;
	bool release = srf_view_release;
	srf_view_release = false;
	bool dying = oracle_view_ref_step(source, release);
	/* A release takes 1 to 0 (no destroy); a promotion takes 0 to 1, or
	 * refuses -1. */
	scratch.ref.refs = release ? 1 : (dying ? -1 : 0);
	scratch.ref.weak_refs = 0;
	scratch.source = source;
	return &scratch;
}

bool obs_source_refs_core_alive(void);
bool obs_source_refs_core_alive(void)
{
	if (srf_current)
		return srf_real_core_alive();
	srf_view_release = true;
	return true;
}

void obs_source_refs_destroy(obs_source_t *source);
void obs_source_refs_destroy(obs_source_t *source)
{
	/* Never reached without a fixture: the scratch control never drops to
	 * -1. */
	if (srf_current)
		srf_real_destroy(source);
}

/* ------------------------------------------------------------------------- */
/* the parity run */

#if defined(__GNUC__) || defined(__clang__)
static void srf_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void srf_capture(int level, const char *format, va_list args, void *param)
{
	struct srf_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*srf_void_call)(void *);
typedef void *(*srf_ptr_call)(void *);
typedef void (*srf_get_log)(log_handler_t *, void **);
typedef void (*srf_set_log)(log_handler_t, void *);

/* The four calls of one side. */
struct srf_calls {
	srf_void_call addref;
	srf_void_call release;
	srf_ptr_call get_ref;
	srf_ptr_call weak_get_source;
};

static void srf_original_addref(void *source)
{
	oracle_obs_source_addref(source);
}

static void srf_original_release(void *source)
{
	oracle_obs_source_release(source);
}

static void *srf_original_get_ref(void *source)
{
	return oracle_obs_source_get_ref(source);
}

static void *srf_original_weak_get_source(void *weak)
{
	return oracle_obs_weak_source_get_source(weak);
}

/* The original calls, for the parity test's other side. */
void oracle_source_refs_originals(struct srf_calls *calls);
void oracle_source_refs_originals(struct srf_calls *calls)
{
	calls->addref = srf_original_addref;
	calls->release = srf_original_release;
	calls->get_ref = srf_original_get_ref;
	calls->weak_get_source = srf_original_weak_get_source;
}

/* Runs one case through `calls` on a fresh noise-filled fixture. False when
 * the fixture could not be built, the case would make the original
 * dereference NULL or free the fixture, or an adapter does not reach its
 * field. */
bool oracle_source_refs_run(const struct srf_calls *calls, srf_get_log get, srf_set_log set, const struct srf_case *c,
			    struct srf_result *out);
bool oracle_source_refs_run(const struct srf_calls *calls, srf_get_log get, srf_set_log set, const struct srf_case *c,
			    struct srf_result *out)
{
	/* The control is not bzalloc'd: the source's own weak release must
	 * never be its last. A missing control is only safe for get_ref. */
	if (c->call > 3 || c->weak_refs < 1 || (c->no_control && c->call != 2))
		return false;

	struct srf_fixture *f = malloc(sizeof(*f));
	struct srf_fixture *before = malloc(sizeof(*before));
	if (!f || !before) {
		free(f);
		free(before);
		return false;
	}
	memset(f, c->noise, sizeof(*f));
	memset(out, 0, sizeof(*out));
	out->log_level = -1;
	f->c = c;
	f->out = out;
	f->control.ref.refs = c->refs;
	f->control.ref.weak_refs = c->weak_refs;
	f->control.source = c->no_source ? NULL : &f->source;
	f->source.context.control = c->no_control ? NULL : (struct obs_weak_object *)&f->control;

	/* The real adapters reach what they name. */
	bool valid = srf_real_control(&f->source) == (c->no_control ? NULL : &f->control) &&
		     obs_weak_source_refs_field(&f->control) == &f->control.ref.refs &&
		     obs_weak_source_source_field(&f->control) == f->control.source;
	memcpy(before, f, sizeof(*f));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	srf_current = f;
	valid = valid && srf_real_core_alive() == c->core_alive;
	set(srf_capture, out);
	void *input = c->call == 3 ? (void *)&f->control : (void *)&f->source;
	if (c->null_input)
		input = NULL;
	void *returned = NULL;
	switch (c->call) {
	case 0:
		calls->addref(input);
		break;
	case 1:
		calls->release(input);
		break;
	case 2:
		returned = calls->get_ref(input);
		break;
	default:
		returned = calls->weak_get_source(input);
		break;
	}
	set(old_handler, old_param);
	srf_current = NULL;

	out->returned = !returned ? 0 : returned == &f->source ? 1 : 2;
	out->refs = f->control.ref.refs;
	out->weak_refs = f->control.ref.weak_refs;
	before->control.ref.refs = f->control.ref.refs;
	before->control.ref.weak_refs = f->control.ref.weak_refs;
	out->untouched = memcmp(&before->core, &f->core, sizeof(f->core)) == 0 &&
			 memcmp(&before->source, &f->source, sizeof(f->source)) == 0 &&
			 memcmp(&before->control, &f->control, sizeof(f->control)) == 0;
	free(before);
	free(f);
	return valid;
}
