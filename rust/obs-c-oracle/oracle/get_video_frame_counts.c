/* Original obs_get_video_frame_counts() and the real-header adapters of
 * obs-get-video-frame-counts-fields.c.h on one recording fixture core.
 * Under cargo test these adapters are the only definitions of their real
 * names, so the Rust shim reaches this fixture. Traced: every read of the
 * core, the mutex lock and unlock, each field the mix rule asks about with
 * its answer (the original's rule calls, the shim's adapter calls), and
 * each frame counter read with the mix it belongs to. The probe runs the
 * original and a candidate on the same fixture and reports, instead of
 * aborting, any difference in the outputs or the trace, a read outside the
 * lock, an output written before the unlock, the mutex left held, or any
 * byte of the canvas, the mix list or the mixes changed, so mutants are
 * observable.
 *
 * Excluded: an uninitialized core, a dead main canvas, a NULL or dead listed
 * mix and a concurrent list change (outside the original's contract; the
 * real lifecycle stays unchanged Tier 2, test_get_video_frame_counts). No
 * intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define VFC_POOL 8
#define VFC_VIEWS 3
#define VFC_LIST 16
#define VFC_TRACE 512
#define VFC_SENTINEL 0xdeadbeefu

/* One pool mix as the Rust side describes it. */
struct oracle_vfc_mix {
	uint8_t encoder_only;
	uint8_t view;
	uint8_t video;
	uint32_t total;
	uint32_t skipped;
};

/* What the fixture core holds: `main` 0 has no main canvas, 1 a canvas
 * without a mix, 2 a canvas whose mix is pool[main_index]. `outputs` 0
 * passes both outputs, 1 a NULL total, 2 a NULL skipped, 3 both NULL and 4
 * one uint32_t for both. */
struct oracle_vfc_case {
	const uint8_t *list;
	size_t count;
	const struct oracle_vfc_mix *mixes;
	uint8_t main;
	uint8_t main_index;
	uint8_t outputs;
};

static _Thread_local struct obs_core vfc_core;
static _Thread_local struct obs_canvas vfc_canvas;
static _Thread_local struct obs_core_video_mix vfc_pool[VFC_POOL];
static _Thread_local struct obs_core_video_mix *vfc_storage[VFC_LIST];
static _Thread_local struct obs_view vfc_views[VFC_VIEWS];
/* Stand-in video_t addresses: only compared, never read. */
static _Thread_local char vfc_videos[VFC_POOL];
static _Thread_local uint32_t vfc_totals[VFC_POOL];
static _Thread_local uint32_t vfc_skips[VFC_POOL];
static _Thread_local uint32_t vfc_out[2];
static _Thread_local bool vfc_locked;
static _Thread_local char vfc_trace[VFC_TRACE];
static _Thread_local size_t vfc_trace_len;

static void vfc_event(char value)
{
	if (vfc_trace_len + 1 < sizeof(vfc_trace)) {
		vfc_trace[vfc_trace_len++] = value;
		vfc_trace[vfc_trace_len] = '\0';
	}
}

/* 'o' reads the core. */
static struct obs_core *vfc_read_core(void)
{
	vfc_event('o');
	return &vfc_core;
}

/* 'L' locks; 'R' marks a recursive lock or another mutex. */
static int vfc_lock(pthread_mutex_t *mutex)
{
	if (mutex != &vfc_core.video.mixes_mutex || vfc_locked)
		vfc_event('R');
	vfc_event('L');
	int result = pthread_mutex_lock(mutex);
	vfc_locked = true;
	return result;
}

/* 'U' unlocks; 'R' marks an unlock without the lock or of another mutex;
 * 'W' an output already written. */
static int vfc_unlock(pthread_mutex_t *mutex)
{
	if (mutex != &vfc_core.video.mixes_mutex || !vfc_locked)
		vfc_event('R');
	if (vfc_out[0] != VFC_SENTINEL || vfc_out[1] != VFC_SENTINEL)
		vfc_event('W');
	vfc_event('U');
	vfc_locked = false;
	return pthread_mutex_unlock(mutex);
}

/* 'X' marks a read outside the lock. */
static void vfc_check_locked(void)
{
	if (!vfc_locked)
		vfc_event('X');
}

/* 't' or 's' then the mix's name reads one of its video's counters; '?' a
 * video that is no mix's. */
static uint32_t vfc_counter(const video_t *video, bool total)
{
	vfc_check_locked();
	for (size_t i = 0; i < VFC_POOL; i++) {
		if (video == (const video_t *)&vfc_videos[i]) {
			vfc_event(total ? 't' : 's');
			vfc_event((char)('0' + i));
			return total ? vfc_totals[i] : vfc_skips[i];
		}
	}
	vfc_event('?');
	return 0;
}

static uint32_t vfc_total(const video_t *video)
{
	return vfc_counter(video, true);
}

static uint32_t vfc_skipped(const video_t *video)
{
	return vfc_counter(video, false);
}

/* A field the rule asked about: 'e' encoder_only_mix, 'v' the same view,
 * 'd' a video, then its answer. */
static void vfc_field(char field, bool value)
{
	vfc_check_locked();
	vfc_event(field);
	vfc_event(value ? '1' : '0');
}

enum obs_core_setup_rules_mix oracle_obs_core_setup_rules_mix_step(enum obs_core_setup_rules_mix step, bool value);

/* The original's rule calls, recorded with the field each one asks about
 * and forwarded to the original rule. */
static enum obs_core_setup_rules_mix vfc_rule(enum obs_core_setup_rules_mix step, bool value)
{
	switch (step) {
	case OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY:
		vfc_field('e', value);
		break;
	case OBS_CORE_SETUP_RULES_MIX_VIEW:
		vfc_field('v', value);
		break;
	case OBS_CORE_SETUP_RULES_MIX_VIDEO:
		vfc_field('d', value);
		break;
	default:
		break;
	}
	return oracle_obs_core_setup_rules_mix_step(step, value);
}

void oracle_obs_get_video_frame_counts(uint32_t *total, uint32_t *skipped);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock vfc_lock
#define pthread_mutex_unlock vfc_unlock
#define video_output_get_total_frames vfc_total
#define video_output_get_skipped_frames vfc_skipped
#define obs vfc_read_core()
#define obs_core_setup_rules_mix_step vfc_rule
#define obs_get_video_frame_counts oracle_obs_get_video_frame_counts
#include "libobs/obs-get-video-frame-counts.c.h"
#undef obs_get_video_frame_counts
#undef obs_core_setup_rules_mix_step
/* The three field adapters are wrapped below to record their answers. */
#define obs_get_video_frame_counts_encoder_only vfc_inner_encoder_only
#define obs_get_video_frame_counts_same_view vfc_inner_same_view
#define obs_get_video_frame_counts_has_video vfc_inner_has_video
#include "get-video-frame-counts-fields.inc"
#undef obs_get_video_frame_counts_has_video
#undef obs_get_video_frame_counts_same_view
#undef obs_get_video_frame_counts_encoder_only
#undef obs
#undef video_output_get_skipped_frames
#undef video_output_get_total_frames
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

bool obs_get_video_frame_counts_encoder_only(const struct obs_core_video_mix *mix);
bool obs_get_video_frame_counts_encoder_only(const struct obs_core_video_mix *mix)
{
	bool value = vfc_inner_encoder_only(mix);
	vfc_field('e', value);
	return value;
}

bool obs_get_video_frame_counts_same_view(const struct obs_core_video_mix *mix,
					  const struct obs_core_video_mix *main_mix);
bool obs_get_video_frame_counts_same_view(const struct obs_core_video_mix *mix,
					  const struct obs_core_video_mix *main_mix)
{
	bool value = vfc_inner_same_view(mix, main_mix);
	/* The original compares with the main canvas's mix only. */
	if (main_mix != vfc_canvas.mix)
		vfc_event('M');
	vfc_field('v', value);
	return value;
}

bool obs_get_video_frame_counts_has_video(const struct obs_core_video_mix *mix);
bool obs_get_video_frame_counts_has_video(const struct obs_core_video_mix *mix)
{
	bool value = vfc_inner_has_video(mix);
	vfc_field('d', value);
	return value;
}

typedef void (*vfc_fn)(uint32_t *total, uint32_t *skipped);

struct vfc_result {
	char trace[VFC_TRACE];
	uint32_t out[2];
	bool untouched;
};

/* The bytes around the read fields are filled, so a write anywhere in a
 * mix, the canvas or a view shows. */
static void vfc_arm(const struct oracle_vfc_case *c)
{
	memset(&vfc_core, 0, sizeof(vfc_core));
	memset(&vfc_canvas, 0x55, sizeof(vfc_canvas));
	memset(vfc_pool, 0x77, sizeof(vfc_pool));
	memset(vfc_views, 0x33, sizeof(vfc_views));
	memset(vfc_storage, 0, sizeof(vfc_storage));
	for (size_t i = 0; i < VFC_POOL; i++) {
		vfc_pool[i].encoder_only_mix = c->mixes[i].encoder_only != 0;
		vfc_pool[i].view = &vfc_views[c->mixes[i].view % VFC_VIEWS];
		vfc_pool[i].video = c->mixes[i].video ? (video_t *)&vfc_videos[i] : NULL;
		vfc_totals[i] = c->mixes[i].total;
		vfc_skips[i] = c->mixes[i].skipped;
	}
	pthread_mutex_init(&vfc_core.video.mixes_mutex, NULL);
	for (size_t i = 0; i < c->count; i++)
		vfc_storage[i] = &vfc_pool[c->list[i] % VFC_POOL];
	vfc_core.video.mixes.array = vfc_storage;
	vfc_core.video.mixes.num = c->count;
	vfc_core.video.mixes.capacity = VFC_LIST;
	vfc_canvas.mix = c->main == 2 ? &vfc_pool[c->main_index % VFC_POOL] : NULL;
	vfc_core.data.main_canvas = c->main ? &vfc_canvas : NULL;
	vfc_out[0] = VFC_SENTINEL;
	vfc_out[1] = VFC_SENTINEL;
	vfc_locked = false;
	vfc_trace_len = 0;
	vfc_trace[0] = '\0';
}

static void vfc_run(vfc_fn counts, const struct oracle_vfc_case *c, struct vfc_result *out)
{
	static _Thread_local struct obs_core_video_mix pool_before[VFC_POOL];
	static _Thread_local struct obs_core_video_mix *storage_before[VFC_LIST];
	static _Thread_local struct obs_canvas canvas_before;
	static _Thread_local struct obs_view views_before[VFC_VIEWS];
	vfc_arm(c);
	memcpy(pool_before, vfc_pool, sizeof(vfc_pool));
	memcpy(storage_before, vfc_storage, sizeof(vfc_storage));
	memcpy(&canvas_before, &vfc_canvas, sizeof(vfc_canvas));
	memcpy(views_before, vfc_views, sizeof(vfc_views));
	struct obs_canvas *main_canvas = vfc_core.data.main_canvas;

	uint32_t *total = &vfc_out[0];
	uint32_t *skipped = &vfc_out[1];
	switch (c->outputs) {
	case 1:
		total = NULL;
		break;
	case 2:
		skipped = NULL;
		break;
	case 3:
		total = NULL;
		skipped = NULL;
		break;
	case 4:
		skipped = total;
		break;
	default:
		break;
	}
	counts(total, skipped);

	memcpy(out->trace, vfc_trace, sizeof(out->trace));
	memcpy(out->out, vfc_out, sizeof(out->out));
	out->untouched = memcmp(vfc_pool, pool_before, sizeof(vfc_pool)) == 0 &&
			 memcmp(vfc_storage, storage_before, sizeof(vfc_storage)) == 0 &&
			 memcmp(&vfc_canvas, &canvas_before, sizeof(vfc_canvas)) == 0 &&
			 memcmp(vfc_views, views_before, sizeof(vfc_views)) == 0 && !vfc_locked &&
			 vfc_core.video.mixes.array == vfc_storage && vfc_core.video.mixes.num == c->count &&
			 vfc_core.video.mixes.capacity == VFC_LIST && vfc_core.data.main_canvas == main_canvas;
	pthread_mutex_destroy(&vfc_core.video.mixes_mutex);
}

/* 0: match. 1: the outputs differ, 2: the traces differ, 4: the candidate
 * (or the original) left the mutex held or changed a byte of the canvas,
 * the list, a view or a mix. 0x100: the list is too long for the fixture. */
unsigned oracle_get_video_frame_counts_probe(vfc_fn candidate, const struct oracle_vfc_case *c);
unsigned oracle_get_video_frame_counts_probe(vfc_fn candidate, const struct oracle_vfc_case *c)
{
	static _Thread_local struct vfc_result original, actual;
	if (c->count > VFC_LIST)
		return 0x100;
	vfc_run(oracle_obs_get_video_frame_counts, c, &original);
	vfc_run(candidate, c, &actual);
	unsigned diff = 0;
	if (memcmp(original.out, actual.out, sizeof(original.out)) != 0)
		diff |= 1;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 2;
	if (!actual.untouched || !original.untouched)
		diff |= 4;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void vfc_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's outputs and trace, for the characterization checks. */
bool oracle_get_video_frame_counts_original(const struct oracle_vfc_case *c, uint32_t *out, char *trace,
					    size_t trace_size);
bool oracle_get_video_frame_counts_original(const struct oracle_vfc_case *c, uint32_t *out, char *trace,
					    size_t trace_size)
{
	static _Thread_local struct vfc_result result;
	if (c->count > VFC_LIST)
		return false;
	vfc_run(oracle_obs_get_video_frame_counts, c, &result);
	vfc_copy(trace, trace_size, result.trace);
	out[0] = result.out[0];
	out[1] = result.out[1];
	return result.untouched;
}

/* The adapters' reads against the real fields. */
bool oracle_get_video_frame_counts_adapters(void);
bool oracle_get_video_frame_counts_adapters(void)
{
	static const uint8_t list[] = {3, 1};
	struct oracle_vfc_mix mixes[VFC_POOL] = {{0}};
	mixes[3] = (struct oracle_vfc_mix){.encoder_only = 1, .view = 2, .video = 1, .total = 7, .skipped = 9};
	mixes[1] = (struct oracle_vfc_mix){.encoder_only = 0, .view = 1, .video = 0};
	mixes[5] = (struct oracle_vfc_mix){.view = 2, .video = 1};
	struct oracle_vfc_case c = {list, 2, mixes, 2, 5, 0};
	vfc_arm(&c);
	bool ok = obs_get_video_frame_counts_main_mix() == &vfc_pool[5];
	obs_get_video_frame_counts_lock();
	ok = ok && vfc_locked && obs_get_video_frame_counts_count() == 2 &&
	     obs_get_video_frame_counts_at(0) == &vfc_pool[3] && obs_get_video_frame_counts_at(1) == &vfc_pool[1] &&
	     vfc_inner_encoder_only(&vfc_pool[3]) && !vfc_inner_encoder_only(&vfc_pool[1]) &&
	     vfc_inner_same_view(&vfc_pool[3], &vfc_pool[5]) && !vfc_inner_same_view(&vfc_pool[1], &vfc_pool[5]) &&
	     vfc_inner_has_video(&vfc_pool[3]) && !vfc_inner_has_video(&vfc_pool[1]) &&
	     obs_get_video_frame_counts_total(&vfc_pool[3]) == 7 &&
	     obs_get_video_frame_counts_skipped(&vfc_pool[3]) == 9;
	obs_get_video_frame_counts_unlock();
	ok = ok && !vfc_locked && strcmp(vfc_trace, "oooLooot3s3oU") == 0;
	pthread_mutex_destroy(&vfc_core.video.mixes_mutex);

	/* No main canvas, then a canvas without a mix. */
	c.main = 0;
	vfc_arm(&c);
	ok = ok && obs_get_video_frame_counts_main_mix() == NULL && strcmp(vfc_trace, "o") == 0;
	pthread_mutex_destroy(&vfc_core.video.mixes_mutex);
	c.main = 1;
	vfc_arm(&c);
	ok = ok && obs_get_video_frame_counts_main_mix() == NULL && strcmp(vfc_trace, "oo") == 0;
	pthread_mutex_destroy(&vfc_core.video.mixes_mutex);
	return ok;
}
