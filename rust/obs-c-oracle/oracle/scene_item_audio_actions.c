/* Tier 3 oracle: the original libobs/obs-scene-item-audio-actions.c.h over
 * the original core (oracle_obs_scene_audio_actions_*, oracle
 * scene_audio_actions.c), and the production field adapter that the Rust
 * shim uses, over the real private layouts.
 *
 * Both are compiled with the lock, the unlock, the three atomics and the
 * child removal renamed to recorders that log each call, in order, into one
 * trace, and then do the real operation. So a probe can compare the order
 * of the calls of the C item side with that of the Rust one, which reaches
 * the same operations through the adapter: what runs under the lock, and
 * that the drops and the removal come after it. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"

enum {
	IAA_LOCK = 1,
	IAA_UNLOCK,
	IAA_LOAD_VISIBLE,
	IAA_SET_VISIBLE,
	IAA_RELEASE,
	IAA_REMOVE,
};

#define IAA_TRACE_MAX 256

struct iaa_event {
	int kind;
	long long value;
};

static struct iaa_event iaa_trace[IAA_TRACE_MAX];
static size_t iaa_trace_len;
static bool iaa_trace_overflow;

/* The scene and the two sources each probe's items point at. */
static struct obs_source iaa_parent_source;
static struct obs_source iaa_child_source;
static struct obs_scene iaa_scene;

static void iaa_record(int kind, long long value)
{
	if (iaa_trace_len < IAA_TRACE_MAX) {
		iaa_trace[iaa_trace_len].kind = kind;
		iaa_trace[iaa_trace_len].value = value;
		iaa_trace_len++;
	} else {
		iaa_trace_overflow = true;
	}
}

static int iaa_lock(pthread_mutex_t *mutex)
{
	iaa_record(IAA_LOCK, 0);
	return pthread_mutex_lock(mutex);
}

/* When set, every unlock then rewrites the time of the item's first queued
 * action, as another thread queueing could, so a read of the queue moved
 * out of the lock sees a different action than the C. Both sides get the
 * same rewrites. */
static bool iaa_clobber;
static uint64_t iaa_clobber_ts;

static int iaa_unlock(pthread_mutex_t *mutex)
{
	iaa_record(IAA_UNLOCK, 0);
	int ret = pthread_mutex_unlock(mutex);
	if (iaa_clobber) {
		struct obs_scene_item *item =
			(struct obs_scene_item *)((char *)mutex - offsetof(struct obs_scene_item, actions_mutex));
		if (item->audio_actions.num)
			item->audio_actions.array[0].timestamp = iaa_clobber_ts;
	}
	return ret;
}

static bool iaa_load_bool(const volatile bool *ptr)
{
	bool value = os_atomic_load_bool(ptr);
	iaa_record(IAA_LOAD_VISIBLE, value);
	return value;
}

static bool iaa_set_bool(volatile bool *ptr, bool value)
{
	iaa_record(IAA_SET_VISIBLE, value);
	return os_atomic_set_bool(ptr, value);
}

static long iaa_dec_long(volatile long *ptr)
{
	long value = os_atomic_dec_long(ptr);
	iaa_record(IAA_RELEASE, value);
	return value;
}

/* The removal both sides call: 1 for the probe's own parent source and
 * child, 0 for anything else. */
void oracle_item_audio_actions_remove(obs_source_t *parent, obs_source_t *child);
void oracle_item_audio_actions_remove(obs_source_t *parent, obs_source_t *child)
{
	iaa_record(IAA_REMOVE, parent == &iaa_parent_source && child == &iaa_child_source);
}

size_t oracle_obs_scene_audio_actions_apply(obs_scene_item_action_array_t *actions, bool *visible, uint64_t ts,
					    size_t sample_rate, float *buf);
bool oracle_obs_scene_audio_action_due(uint64_t timestamp, uint64_t ts, size_t sample_rate);
void oracle_apply_scene_item_audio_actions(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate);
bool oracle_apply_scene_item_volume(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate);
void oracle_process_all_audio_actions(struct obs_scene_item *item, size_t sample_rate);

/* Keep any command-line pthread renames for the code after this. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock iaa_lock
#define pthread_mutex_unlock iaa_unlock
#define os_atomic_load_bool iaa_load_bool
#define os_atomic_set_bool iaa_set_bool
#define os_atomic_dec_long iaa_dec_long
#define obs_source_remove_active_child oracle_item_audio_actions_remove
#define obs_scene_audio_actions_apply oracle_obs_scene_audio_actions_apply
#define obs_scene_audio_action_due oracle_obs_scene_audio_action_due
#define apply_scene_item_audio_actions oracle_apply_scene_item_audio_actions
#define apply_scene_item_volume oracle_apply_scene_item_volume
#define process_all_audio_actions oracle_process_all_audio_actions
#include "scene_item_audio_actions.inc"
#undef process_all_audio_actions
#undef apply_scene_item_volume
#undef apply_scene_item_audio_actions
#undef obs_scene_audio_action_due
#undef obs_scene_audio_actions_apply
#undef obs_source_remove_active_child
#include "scene_item_audio_actions_fields.inc"
#undef os_atomic_dec_long
#undef os_atomic_set_bool
#undef os_atomic_load_bool
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

typedef void (*iaa_apply_fn)(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate);
typedef bool (*iaa_volume_fn)(struct obs_scene_item *item, float *buf, uint64_t ts, size_t sample_rate);
typedef void (*iaa_all_fn)(struct obs_scene_item *item, size_t sample_rate);

/* What a probe compares: the item after the call, the gains (with one past
 * the tick), the return value and the trace. */
struct iaa_side {
	struct obs_scene_item item;
	size_t pushed;
	float buf[AUDIO_OUTPUT_FRAMES + 1];
	bool ret;
	struct iaa_event trace[IAA_TRACE_MAX];
	size_t trace_len;
	bool trace_overflow;
};

/* A gain neither side writes: neither 0 nor 1. */
#define IAA_UNWRITTEN 0.5f

static bool iaa_side_init(struct iaa_side *side, const struct item_action *actions, size_t count, bool visible,
			  long refs)
{
	memset(side, 0, sizeof(*side));
	side->item.parent = &iaa_scene;
	side->item.source = &iaa_child_source;
	side->item.visible = visible;
	side->item.active_refs = refs;
	for (size_t i = 0; i <= AUDIO_OUTPUT_FRAMES; i++)
		side->buf[i] = IAA_UNWRITTEN;
	for (size_t i = 0; i < count; i++) {
		/* zeroed, so the padding both queues hold is the same */
		struct item_action action;
		memset(&action, 0, sizeof(action));
		action.visible = actions[i].visible;
		action.timestamp = actions[i].timestamp;
		da_push_back(side->item.audio_actions, &action);
	}
	side->pushed = count;
	return pthread_mutex_init(&side->item.actions_mutex, NULL) == 0;
}

static void iaa_side_run(struct iaa_side *side, unsigned op, uint64_t ts, size_t sample_rate, bool with_buf,
			 bool clobber, uint64_t clobber_ts, iaa_apply_fn apply, iaa_volume_fn volume, iaa_all_fn all)
{
	iaa_clobber = clobber;
	iaa_clobber_ts = clobber_ts;
	float *buf = with_buf ? side->buf : NULL;
	iaa_trace_len = 0;
	iaa_trace_overflow = false;
	switch (op) {
	case 0:
		apply(&side->item, buf, ts, sample_rate);
		break;
	case 1:
		side->ret = volume(&side->item, buf, ts, sample_rate);
		break;
	default:
		all(&side->item, sample_rate);
		break;
	}
	iaa_clobber = false;
	memcpy(side->trace, iaa_trace, sizeof(side->trace));
	side->trace_len = iaa_trace_len;
	side->trace_overflow = iaa_trace_overflow;
}

static void iaa_side_free(struct iaa_side *side)
{
	da_free(side->item.audio_actions);
	pthread_mutex_destroy(&side->item.actions_mutex);
}

/* Bits of the probe's result. */
enum {
	IAA_DIFF_RETURN = 1 << 0,
	IAA_DIFF_VISIBLE = 1 << 1,
	IAA_DIFF_REFS = 1 << 2,
	IAA_DIFF_QUEUE = 1 << 3,
	IAA_DIFF_GAINS = 1 << 4,
	IAA_DIFF_TRACE = 1 << 5,
	IAA_BAD_INPUT = 1 << 6,
	IAA_SETUP_FAILED = 1 << 7,
};

static unsigned iaa_compare(const struct iaa_side *c, const struct iaa_side *rust)
{
	unsigned diff = 0;
	if (c->ret != rust->ret)
		diff |= IAA_DIFF_RETURN;
	if (c->item.visible != rust->item.visible)
		diff |= IAA_DIFF_VISIBLE;
	if (c->item.active_refs != rust->item.active_refs)
		diff |= IAA_DIFF_REFS;
	/* every item the queue held, the stale ones past num included (the
	 * capacity past them was never written) */
	if (c->item.audio_actions.num != rust->item.audio_actions.num ||
	    c->item.audio_actions.capacity != rust->item.audio_actions.capacity ||
	    (c->pushed && memcmp(c->item.audio_actions.array, rust->item.audio_actions.array,
				 c->pushed * sizeof(struct item_action))))
		diff |= IAA_DIFF_QUEUE;
	if (memcmp(c->buf, rust->buf, sizeof(c->buf)))
		diff |= IAA_DIFF_GAINS;
	if (c->trace_overflow || rust->trace_overflow || c->trace_len != rust->trace_len ||
	    memcmp(c->trace, rust->trace, c->trace_len * sizeof(c->trace[0])))
		diff |= IAA_DIFF_TRACE;
	return diff;
}

/* Runs op (0 apply_scene_item_audio_actions, 1 apply_scene_item_volume,
 * 2 process_all_audio_actions) on two items made from the same queue,
 * visibility and references: the original C on one, the Rust callbacks on
 * the other. Returns 0 when everything matches, else the IAA_DIFF_* bits.
 * With clobber, each unlock rewrites the first queued action's time to
 * clobber_ts (iaa_unlock).
 *
 * The core's intentional differences are refused (IAA_BAD_INPUT): a buffer
 * with ts 0, where the C writes past the tick, and a rate of 0 for the
 * window, where the C divides by 0. */
unsigned oracle_item_audio_actions_probe(unsigned op, const struct item_action *actions, size_t count, bool visible,
					 long refs, uint64_t ts, size_t sample_rate, bool with_buf, bool clobber,
					 uint64_t clobber_ts, iaa_apply_fn apply, iaa_volume_fn volume, iaa_all_fn all);
unsigned oracle_item_audio_actions_probe(unsigned op, const struct item_action *actions, size_t count, bool visible,
					 long refs, uint64_t ts, size_t sample_rate, bool with_buf, bool clobber,
					 uint64_t clobber_ts, iaa_apply_fn apply, iaa_volume_fn volume, iaa_all_fn all)
{
	if (op > 2 || (with_buf && (ts == 0 || op == 2)) || (op != 0 && sample_rate == 0))
		return IAA_BAD_INPUT;

	static struct iaa_side c, rust;
	iaa_scene.source = &iaa_parent_source;

	unsigned result = 0;
	bool c_ok = iaa_side_init(&c, actions, count, visible, refs);
	bool rust_ok = iaa_side_init(&rust, actions, count, visible, refs);
	if (c_ok && rust_ok) {
		iaa_side_run(&c, op, ts, sample_rate, with_buf, clobber, clobber_ts,
			     oracle_apply_scene_item_audio_actions, oracle_apply_scene_item_volume,
			     oracle_process_all_audio_actions);
		iaa_side_run(&rust, op, ts, sample_rate, with_buf, clobber, clobber_ts, apply, volume, all);
		result = iaa_compare(&c, &rust);
	} else {
		result = IAA_SETUP_FAILED;
	}
	iaa_side_free(&c);
	iaa_side_free(&rust);
	return result;
}
