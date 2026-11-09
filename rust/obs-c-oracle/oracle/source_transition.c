/* Test-only oracle: the original libobs/obs-source-transition.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims (rust/obs-source-seam/src/ffi/source_transition.rs).
 *
 * The C reads the transition and child fields and takes transition_mutex
 * directly; the Rust shims reach the same fields and lock through the
 * transition seam, compiled under libobs's names by
 * oracle/source_transition_settings.c. The rules it asks
 * (obs_source_transition_render_audio_*, _sample_time, _clock_min_ts) and
 * transition_valid() are the ones the hybrid libobs has. Its stop helpers
 * are the stop oracle's C (oracle/source_transition_stop.c), renamed here
 * too, so the C side is the original C throughout.
 *
 * Its calls into the rest of libobs go to recording stand-ins, which the
 * parity test hands the Rust shims too (their Calls), so both sides run on
 * the slots oracle's graph (oracle/source_transition_slots.c): its
 * obs_source_get_ref() and obs_source_release(), and here
 * obs_source_removed(), obs_source_get_audio_timestamp(),
 * obs_source_get_audio_mix(), gs_texrender_create(), gs_texrender_destroy(),
 * gs_enter_context() and gs_leave_context(), each traced in the slots
 * oracle's trace in order with the others. The core's graphics context,
 * obs->video.graphics, is this file's own core's: the renames start with
 * obs, so here it is oracle_transition_audio_obs, whose graphics pointer
 * is a stand-in. A child's audio is a block of TOTAL_AUDIO_SIZE bytes this
 * file allocates, laid out as libobs's (allocate_audio_output_buffer()).
 *
 * The type's two mix callbacks are the recording callbacks below: each
 * counts its calls, folds every time it gets into a hash, records whether
 * the watched transition's transition_mutex was held, and returns a value
 * of the time and the call count, so a wrong sample order changes the
 * mix. */

#define obs oracle_transition_audio_obs

#define obs_transition_init oracle_obs_transition_init
#define obs_transition_free oracle_obs_transition_free
#define obs_transition_audio_render oracle_obs_transition_audio_render

/* obs-internal.h and the group's headers come first, unrenamed (the
 * transition_stop member of struct obs_source_info keeps its name), and
 * the stop helpers are renamed after them, with their own prototypes. */
#include "libobs/build/libobs-ported-source-transition.pch.h"

float oracle_transition_calc_time(obs_source_t *transition, uint64_t ts);
void oracle_transition_stop(obs_source_t *transition);
void oracle_transition_handle_stop(obs_source_t *transition);

obs_source_t *oracle_slots_get_ref(obs_source_t *source);
void oracle_slots_release(obs_source_t *source);
void oracle_slots_trace(int kind, obs_source_t *a, obs_source_t *b, int result);

bool oracle_audio_removed(obs_source_t *source);
uint64_t oracle_audio_timestamp(obs_source_t *source);
void oracle_audio_get_mix(obs_source_t *source, struct obs_source_audio_mix *audio);
gs_texrender_t *oracle_audio_texrender_create(enum gs_color_format format, enum gs_zstencil_format zsformat);
void oracle_audio_texrender_destroy(gs_texrender_t *texrender);
void oracle_audio_enter_context(graphics_t *graphics);
void oracle_audio_leave_context(void);

#define transition_calc_time oracle_transition_calc_time
#define transition_stop oracle_transition_stop
#define transition_handle_stop oracle_transition_handle_stop
#define obs_source_get_ref oracle_slots_get_ref
#define obs_source_release oracle_slots_release
#define obs_source_removed oracle_audio_removed
#define obs_source_get_audio_timestamp oracle_audio_timestamp
#define obs_source_get_audio_mix oracle_audio_get_mix
#define gs_texrender_create oracle_audio_texrender_create
#define gs_texrender_destroy oracle_audio_texrender_destroy
#define gs_enter_context oracle_audio_enter_context
#define gs_leave_context oracle_audio_leave_context

#include "obs-source-transition.c.h"

#undef gs_leave_context
#undef gs_enter_context
#undef gs_texrender_destroy
#undef gs_texrender_create
#undef obs_source_get_audio_mix
#undef obs_source_get_audio_timestamp
#undef obs_source_removed
#undef obs_source_release
#undef obs_source_get_ref
#undef transition_handle_stop
#undef transition_stop
#undef transition_calc_time

/* The trace kinds of the stand-ins below, after the slots oracle's 1-4, the
 * switch oracle's 5-11 and the stop oracle's 12-13. */
#define ORACLE_AUDIO_REMOVED 20
#define ORACLE_AUDIO_TIMESTAMP 21
#define ORACLE_AUDIO_GET_MIX 22
#define ORACLE_AUDIO_TEX_CREATE 23
#define ORACLE_AUDIO_TEX_DESTROY 24
#define ORACLE_AUDIO_ENTER 25
#define ORACLE_AUDIO_LEAVE 26

#define ORACLE_AUDIO_BLOCKS 16

static struct obs_core audio_core;
struct obs_core *oracle_transition_audio_obs = &audio_core;

static struct {
	obs_source_t *watched;
	/* the texrenders the creates hand out: 1-based ids, 0 for NULL */
	int creates[2];
	size_t created;
	char texrenders[2];
	float *blocks[ORACLE_AUDIO_BLOCKS];
	size_t num_blocks;
	struct {
		long long calls;
		uint64_t hash;
		long long held;
	} mix[2];
	bool clobber;
} au;

/* A recognizable graphics context: never dereferenced. */
static char audio_graphics;

void oracle_audio_reset(obs_source_t *watched, int create_a, int create_b);
void oracle_audio_child(obs_source_t *child, uint64_t ts, bool pending, bool duplicated, bool removed, uint32_t seed);
float *oracle_audio_block_new(uint32_t seed);
gs_texrender_t *oracle_audio_texrender(int id);
graphics_t *oracle_audio_graphics(void);
void oracle_audio_mix_record(int which, long long *calls, uint64_t *hash, long long *held);
void oracle_audio_clobber(bool on);
float oracle_audio_mix_a(void *data, float t);
float oracle_audio_mix_b(void *data, float t);
size_t oracle_audio_layout(int what);
int oracle_audio_constant(int what);

int oracle_stop_held(obs_source_t *transition);

static void trace(int kind, obs_source_t *a, obs_source_t *b, int result)
{
	oracle_slots_trace(kind, a, b, result);
}

/* A block of every mix and channel, its samples from seed (exact floats). */
float *oracle_audio_block_new(uint32_t seed)
{
	float *block;

	if (au.num_blocks == ORACLE_AUDIO_BLOCKS)
		return NULL;
	block = bzalloc(TOTAL_AUDIO_SIZE);
	for (size_t i = 0; i < TOTAL_AUDIO_SIZE / sizeof(float); i++) {
		uint32_t x = seed * 2654435761u + (uint32_t)i * 40503u;
		block[i] = (float)((int)(x >> 20) - 2048) / 1024.0f;
	}
	au.blocks[au.num_blocks++] = block;
	return block;
}

/* Frees the blocks and watches `watched`; the next texrender creates hand
 * out ids create_a, then create_b (0 for NULL). */
void oracle_audio_reset(obs_source_t *watched, int create_a, int create_b)
{
	for (size_t i = 0; i < au.num_blocks; i++)
		bfree(au.blocks[i]);
	memset(&au, 0, sizeof(au));
	au.watched = watched;
	au.creates[0] = create_a;
	au.creates[1] = create_b;
	audio_core.video.graphics = (graphics_t *)&audio_graphics;
}

/* A child's audio state: its timestamp, flags and a block of its own laid
 * out as libobs lays it out. */
void oracle_audio_child(obs_source_t *child, uint64_t ts, bool pending, bool duplicated, bool removed, uint32_t seed)
{
	float *block = oracle_audio_block_new(seed);

	child->audio_ts = ts;
	child->audio_pending = pending;
	child->audio_is_duplicated = duplicated;
	child->removed = removed;
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++) {
			child->audio_output_buf[mix][ch] =
				block ? block + (mix * MAX_AUDIO_CHANNELS + ch) * AUDIO_OUTPUT_FRAMES : NULL;
		}
	}
}

/* The texrender of id 1 or 2, NULL for 0. */
gs_texrender_t *oracle_audio_texrender(int id)
{
	return id >= 1 && id <= 2 ? (gs_texrender_t *)&au.texrenders[id - 1] : NULL;
}

graphics_t *oracle_audio_graphics(void)
{
	return (graphics_t *)&audio_graphics;
}

static int texrender_id(const gs_texrender_t *texrender)
{
	for (int i = 0; i < 2; i++) {
		if (texrender == (const gs_texrender_t *)&au.texrenders[i])
			return i + 1;
	}
	return texrender ? 99 : 0;
}

/* The stand-ins */

bool oracle_audio_removed(obs_source_t *source)
{
	trace(ORACLE_AUDIO_REMOVED, source, NULL, source->removed);
	return source->removed;
}

uint64_t oracle_audio_timestamp(obs_source_t *source)
{
	trace(ORACLE_AUDIO_TIMESTAMP, source, NULL, oracle_stop_held(au.watched));
	return source->audio_ts;
}

void oracle_audio_get_mix(obs_source_t *source, struct obs_source_audio_mix *audio)
{
	trace(ORACLE_AUDIO_GET_MIX, source, NULL, oracle_stop_held(au.watched));
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			audio->output[mix].data[ch] = source->audio_output_buf[mix][ch];
	}
}

gs_texrender_t *oracle_audio_texrender_create(enum gs_color_format format, enum gs_zstencil_format zsformat)
{
	int id = au.created < 2 ? au.creates[au.created] : 0;

	au.created++;
	trace(ORACLE_AUDIO_TEX_CREATE, NULL, NULL, id * 100 + (int)format * 10 + (int)zsformat);
	return oracle_audio_texrender(id);
}

void oracle_audio_texrender_destroy(gs_texrender_t *texrender)
{
	trace(ORACLE_AUDIO_TEX_DESTROY, NULL, NULL, texrender_id(texrender));
}

void oracle_audio_enter_context(graphics_t *graphics)
{
	trace(ORACLE_AUDIO_ENTER, NULL, NULL, graphics == (graphics_t *)&audio_graphics ? 1 : graphics ? 99 : 0);
}

void oracle_audio_leave_context(void)
{
	trace(ORACLE_AUDIO_LEAVE, NULL, NULL, 0);
}

/* The mix callbacks: data must be the watched transition's context data. */
static float mix_cb(int which, void *data, float t)
{
	uint32_t bits;

	memcpy(&bits, &t, sizeof(bits));
	au.mix[which].hash = (au.mix[which].hash ^ bits) * 1099511628211ull;
	/* The data as its offset from the watched transition (1-based, 0 for
	 * NULL): the C and Rust sides run on graphs allocated one after the
	 * other, which need not share addresses. */
	au.mix[which].hash ^= data ? (uint64_t)((uintptr_t)data - (uintptr_t)au.watched) + 1 : 0;
	au.mix[which].held += oracle_stop_held(au.watched);
	au.mix[which].calls++;
	if (au.clobber) {
		/* As another thread may once the render let the lock go: a new
		 * clock, the audio flag down and new context data, each call. */
		obs_source_t *tr = au.watched;
		tr->transition_mode = OBS_TRANSITION_MODE_AUTO;
		tr->transition_manual_val = 0.75f;
		tr->transition_start_time += 5;
		tr->transition_duration = 7;
		os_atomic_set_bool(&tr->transitioning_audio, false);
		if (tr->context.data)
			tr->context.data = (char *)tr->context.data + 1;
	}
	return t * 0.5f + (float)(au.mix[which].calls % 7) * 0.125f - 0.25f + (float)which;
}

float oracle_audio_mix_a(void *data, float t)
{
	return mix_cb(0, data, t);
}

float oracle_audio_mix_b(void *data, float t)
{
	return mix_cb(1, data, t);
}

/* From now on each mix callback call rewrites the watched transition's
 * clock, audio flag and context data. */
void oracle_audio_clobber(bool on)
{
	au.clobber = on;
}

void oracle_audio_mix_record(int which, long long *calls, uint64_t *hash, long long *held)
{
	*calls = au.mix[which].calls;
	*hash = au.mix[which].hash;
	*held = au.mix[which].held;
}

/* The layout of struct obs_source_audio_mix and struct audio_output_data:
 * 0 size, 1 align, 2 the offset of output[1], 3 the size and 4 the align of
 * audio_output_data, 5 the offset of its data[1]. */
size_t oracle_audio_layout(int what)
{
	switch (what) {
	case 0:
		return sizeof(struct obs_source_audio_mix);
	case 1:
		return _Alignof(struct obs_source_audio_mix);
	case 2:
		return offsetof(struct obs_source_audio_mix, output[1]);
	case 3:
		return sizeof(struct audio_output_data);
	case 4:
		return _Alignof(struct audio_output_data);
	case 5:
		return offsetof(struct audio_output_data, data[1]);
	default:
		return 0;
	}
}

/* The constants the shims pass as numbers: 0 GS_RGBA, 1 GS_ZS_NONE, 2
 * OBS_ALIGN_LEFT | OBS_ALIGN_TOP, 3 MAX_AUDIO_MIXES, 4 MAX_AUDIO_CHANNELS,
 * 5 AUDIO_OUTPUT_FRAMES, 6 TOTAL_AUDIO_SIZE. */
int oracle_audio_constant(int what)
{
	switch (what) {
	case 0:
		return GS_RGBA;
	case 1:
		return GS_ZS_NONE;
	case 2:
		return OBS_ALIGN_LEFT | OBS_ALIGN_TOP;
	case 3:
		return MAX_AUDIO_MIXES;
	case 4:
		return MAX_AUDIO_CHANNELS;
	case 5:
		return AUDIO_OUTPUT_FRAMES;
	case 6:
		return (int)TOTAL_AUDIO_SIZE;
	default:
		return -1;
	}
}

void oracle_audio_mutexes(obs_source_t *source, bool make);
uint32_t oracle_audio_alignment(const obs_source_t *transition);

/* Destroys (make false) or makes (true) both transition mutexes, so init
 * starts from a source whose mutexes are not made yet and the graph's
 * reset destroys each once. */
void oracle_audio_mutexes(obs_source_t *source, bool make)
{
	if (make) {
		pthread_mutex_init(&source->transition_mutex, NULL);
		pthread_mutex_init(&source->transition_tex_mutex, NULL);
	} else {
		pthread_mutex_destroy(&source->transition_mutex);
		pthread_mutex_destroy(&source->transition_tex_mutex);
	}
}

uint32_t oracle_audio_alignment(const obs_source_t *transition)
{
	return transition->transition_alignment;
}
