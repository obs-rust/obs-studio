/* Test-only oracle: the original libobs/obs-audio.c.h, unmodified, with
 * audio_callback() renamed to oracle_audio_callback() so it can link next to
 * the Rust shim (rust/obs-core/src/ffi/audio_callback.rs).
 *
 * Its decisions are the C oracles' (audio_render.c, audio_mix.c,
 * audio_discard.c, audio_min_ts.c, under their oracle_* names), so this side
 * is C throughout. The core-audio seam, libobs/obs-audio-seam.c.h, is
 * compiled here under its own names, as it stays C in libobs.core-output:
 * the Rust shim reaches the fields through it.
 *
 * Both sides run on a world of their own: a zeroed struct obs_core with its
 * mutexes, its views, mixes and sources. `obs` (renamed oracle_tick_obs)
 * points at the world that runs; the Rust parity test runs one world at a
 * time, under its own lock. The calls into the rest of libobs
 * (audio_output_get_sample_rate/_channels, obs_source_get_ref, _release,
 * _get_name, _removed, _active, _audio_render, _enum_active_tree, blog) are
 * renamed to the recording stand-ins below, which the parity test hands the
 * Rust shim too (its Calls): each one appends to the world's log, with the
 * locks it sees held where that matters. obs_source_get_name() is not logged:
 * the C evaluates the log arguments in either order.
 *
 * A test source is a zeroed struct obs_source with its mutex, its input
 * deques and output buffers, and a struct tick_src in context.data. */

#include <stddef.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define ORACLE_TICK_SOURCES 12
#define ORACLE_TICK_VIEWS 4
#define ORACLE_TICK_MIXES 4
#define ORACLE_TICK_TASKS 16
#define ORACLE_TICK_LOG 131072

/* The core libobs keeps in obs.c, under its own name. */
extern struct obs_core *oracle_tick_obs;

uint32_t oracle_tick_sample_rate(const audio_t *audio);
size_t oracle_tick_channels(const audio_t *audio);
obs_source_t *oracle_tick_get_ref(obs_source_t *source);
void oracle_tick_release(obs_source_t *source);
const char *oracle_tick_get_name(const obs_source_t *source);
bool oracle_tick_removed(const obs_source_t *source);
bool oracle_tick_active(const obs_source_t *source);
void oracle_tick_audio_render(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate, size_t size);
void oracle_tick_enum_active_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param);
void oracle_tick_blog(int log_level, const char *format, ...);

bool oracle_audio_callback(void *param, uint64_t start_ts_in, uint64_t end_ts_in, uint64_t *out_ts, uint32_t mixers,
			   struct audio_output_data *mixes);

struct obs_audio_ignore_step oracle_obs_audio_discard_ignore(const struct obs_audio_input *in, size_t channels,
							     size_t sample_rate, uint64_t start_ts);
struct obs_audio_discard_step oracle_obs_audio_discard_tick(const struct obs_audio_input *in, bool audio_render,
							    size_t channels, size_t sample_rate,
							    const struct ts_info *ts);
size_t oracle_obs_audio_mix_time_to_frames(size_t sample_rate, uint64_t t);
void oracle_obs_audio_mix_add(struct audio_output_data *mixes, float *(*buf)[MAX_AUDIO_CHANNELS], uint64_t audio_ts,
			      const struct ts_info *ts, size_t channels, size_t sample_rate);
void oracle_obs_audio_mix_add_mixers(struct audio_output_data *mixes, float *(*buf)[MAX_AUDIO_CHANNELS],
				     uint64_t audio_ts, const struct ts_info *ts, size_t channels, size_t sample_rate,
				     uint32_t mixers);
bool oracle_obs_audio_buffering_maxed(const struct obs_audio_buffering *buffering);
struct obs_audio_buffering_step oracle_obs_audio_buffering_fixed(const struct obs_audio_buffering *buffering,
								 size_t sample_rate, struct ts_info *ts);
struct obs_audio_buffering_step oracle_obs_audio_buffering_add(const struct obs_audio_buffering *buffering,
							       size_t sample_rate, struct ts_info *ts, uint64_t min_ts);
enum obs_audio_tree_choice oracle_obs_audio_render_tree(bool removed, bool present, uint32_t type,
							uint32_t output_flags, bool duplicated);
bool oracle_obs_audio_render_silence(const struct obs_audio_monitor_dedup *dedup, bool monitoring_enabled,
				     bool is_dedup_source);
uint32_t oracle_obs_audio_render_clear_mixes(uint32_t dedup_mixers, uint32_t source_mixers);
enum obs_audio_backwards_choice oracle_obs_audio_render_backwards(bool maxed, uint64_t audio_ts, uint64_t tick_start,
								  bool audio_render);
bool oracle_obs_audio_min_ts_earlier(bool pending, uint64_t audio_ts, uint64_t *min_ts);
bool oracle_obs_audio_min_ts_insufficient(bool audio_render, bool *pending, uint64_t audio_ts, size_t buffered,
					  size_t sample_rate, uint64_t min_ts);
enum obs_audio_buffering_choice oracle_obs_audio_min_ts_buffering(bool fixed_buffer, bool maxed, uint64_t min_ts,
								  uint64_t tick_start);

#define obs oracle_tick_obs
#define audio_callback oracle_audio_callback
#define audio_output_get_sample_rate oracle_tick_sample_rate
#define audio_output_get_channels oracle_tick_channels
#define obs_source_get_ref oracle_tick_get_ref
#define obs_source_release oracle_tick_release
#define obs_source_get_name oracle_tick_get_name
#define obs_source_removed oracle_tick_removed
#define obs_source_active oracle_tick_active
#define obs_source_audio_render oracle_tick_audio_render
#define obs_source_enum_active_tree oracle_tick_enum_active_tree
#define blog oracle_tick_blog
#define obs_audio_discard_ignore oracle_obs_audio_discard_ignore
#define obs_audio_discard_tick oracle_obs_audio_discard_tick
#define obs_audio_mix_time_to_frames oracle_obs_audio_mix_time_to_frames
#define obs_audio_mix_add oracle_obs_audio_mix_add
#define obs_audio_mix_add_mixers oracle_obs_audio_mix_add_mixers
#define obs_audio_buffering_maxed oracle_obs_audio_buffering_maxed
#define obs_audio_buffering_fixed oracle_obs_audio_buffering_fixed
#define obs_audio_buffering_add oracle_obs_audio_buffering_add
#define obs_audio_render_tree oracle_obs_audio_render_tree
#define obs_audio_render_silence oracle_obs_audio_render_silence
#define obs_audio_render_clear_mixes oracle_obs_audio_render_clear_mixes
#define obs_audio_render_backwards oracle_obs_audio_render_backwards
#define obs_audio_min_ts_earlier oracle_obs_audio_min_ts_earlier
#define obs_audio_min_ts_insufficient oracle_obs_audio_min_ts_insufficient
#define obs_audio_min_ts_buffering oracle_obs_audio_min_ts_buffering

#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wconversion"
#endif

#include "obs-audio.c.h"

#undef obs_audio_min_ts_buffering
#undef obs_audio_min_ts_insufficient
#undef obs_audio_min_ts_earlier
#undef obs_audio_render_backwards
#undef obs_audio_render_clear_mixes
#undef obs_audio_render_silence
#undef obs_audio_render_tree
#undef obs_audio_buffering_add
#undef obs_audio_buffering_fixed
#undef obs_audio_buffering_maxed
#undef obs_audio_mix_add_mixers
#undef obs_audio_mix_add
#undef obs_audio_mix_time_to_frames
#undef obs_audio_discard_tick
#undef obs_audio_discard_ignore
#undef blog
#undef obs_source_enum_active_tree
#undef obs_source_audio_render
#undef obs_source_active
#undef obs_source_removed
#undef obs_source_get_name
#undef obs_source_release
#undef obs_source_get_ref
#undef audio_output_get_channels
#undef audio_output_get_sample_rate
#undef audio_callback

/* `obs` stays renamed: the seam reads the world that runs. */
#include "obs-audio-seam.c.h"

#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

#undef obs

struct obs_core *oracle_tick_obs;

/* A test source's own state. */
struct tick_src {
	int id;
	char name[8];
	bool removed;
	bool active;
	/* obs_source_get_ref() gives NULL: the source is being destroyed */
	bool dead;
	int refs;
	int tree[ORACLE_TICK_SOURCES];
	size_t tree_len;
	uint32_t renders;
};

struct tick_task {
	struct oracle_tick_world *world;
	int id;
	/* the task queues task id + 100 when it runs, this many times down */
	int chain;
};

struct oracle_tick_world {
	struct obs_core core;
	obs_source_t *src[ORACLE_TICK_SOURCES];
	size_t src_len;
	struct obs_view views[ORACLE_TICK_VIEWS];
	size_t view_len;
	struct obs_core_video_mix *mixes[ORACLE_TICK_MIXES];
	struct audio_output_data out[MAX_AUDIO_MIXES];
	struct tick_task tasks[ORACLE_TICK_TASKS];
	size_t task_len;
	uint32_t rate;
	size_t channels;
	/* what audio_callback() writes *out_ts to; the tasks log it */
	uint64_t out_ts;
	char log[ORACLE_TICK_LOG];
	size_t len;
};

/* The starting state of a test source. */
struct oracle_tick_source_init {
	uint32_t kind;
	uint32_t output_flags;
	bool audio_render;
	bool removed;
	bool active;
	bool dead;
	bool pending;
	bool pending_stop;
	bool timing_set;
	bool duplicated;
	bool muted;
	bool monitoring;
	uint64_t audio_ts;
	uint32_t frames;
	/* the mixes whose output buffers exist */
	uint32_t out_mixes;
	uint32_t audio_mixers;
	float volume;
};

/* The core's buffering fields. */
struct oracle_tick_core_init {
	uint64_t buffered_ts;
	uint64_t buffering_wait_ticks;
	int total_buffering_ticks;
	int max_buffering_ticks;
	bool fixed_buffer;
};

void *oracle_tick_new(uint32_t rate, size_t channels, const struct oracle_tick_core_init *init);
void oracle_tick_free(void *world);
int oracle_tick_add_source(void *world, const struct oracle_tick_source_init *init);
void oracle_tick_set_tree(void *world, int source, const int *tree, size_t len);
void oracle_tick_link_audio(void *world, const int *order, size_t len);
int oracle_tick_add_view(void *world);
void oracle_tick_set_channel(void *world, int view, uint32_t channel, int source);
void oracle_tick_add_mix(void *world, int view, bool mix_audio);
void oracle_tick_set_dedup(void *world, int source);
void oracle_tick_queue_task(void *world, int id, int chain);
void oracle_tick_feed(void *world, int source, uint32_t frames, uint64_t audio_ts);
void oracle_tick_use(void *world);
struct audio_output_data *oracle_tick_clear_mixes(void *world);
uint64_t *oracle_tick_out_ts(void *world);
bool oracle_tick_run_c(void *world, uint64_t start, uint64_t end, uint32_t mixers, uint64_t *out_ts);
size_t oracle_tick_state(void *world, char *out, size_t cap);

static struct oracle_tick_world *current;

/* Set as info.audio_render; the tick only checks it is set. */
static bool dummy_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio_output, uint32_t mixers,
			       size_t channels, size_t sample_rate)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(ts_out);
	UNUSED_PARAMETER(audio_output);
	UNUSED_PARAMETER(mixers);
	UNUSED_PARAMETER(channels);
	UNUSED_PARAMETER(sample_rate);
	return false;
}

/* No <stdio.h>: on MSVC it makes this object define snprintf(), and another
 * test binary that links this library then pulls the object in for it.
 * Values are written as the hex of their bits, so they compare exactly. */
static void put_str(char *out, size_t cap, size_t *len, const char *text)
{
	size_t n = strlen(text);

	if (*len + n < cap) {
		memcpy(out + *len, text, n);
		*len += n;
		out[*len] = 0;
	}
}

static void put_hex(char *out, size_t cap, size_t *len, uint64_t value)
{
	char text[17];

	for (int i = 0; i < 16; i++)
		text[i] = "0123456789abcdef"[(value >> ((15 - i) * 4)) & 15];
	text[16] = 0;
	put_str(out, cap, len, text);
}

static uint64_t fnv_add(uint64_t hash, const void *data, size_t size)
{
	const uint8_t *bytes = data;

	for (size_t i = 0; i < size; i++) {
		hash ^= bytes[i];
		hash *= 0x100000001b3ULL;
	}
	return hash;
}

static void log_str(const char *text)
{
	put_str(current->log, sizeof(current->log), &current->len, text);
}

static void log_hex(uint64_t value)
{
	put_hex(current->log, sizeof(current->log), &current->len, value);
}

static struct tick_src *src_of(const obs_source_t *source)
{
	return source->context.data;
}

static int id_of(const obs_source_t *source)
{
	return source ? src_of(source)->id : -1;
}

static void log_src(const char *what, const obs_source_t *source)
{
	log_str(what);
	log_str(" ");
	log_hex((uint64_t)(int64_t)id_of(source));
	log_str(";");
}

/* Whether a mutex is free, seen from a stand-in. */
static bool mutex_free(pthread_mutex_t *mutex)
{
	if (pthread_mutex_trylock(mutex) != 0)
		return false;
	pthread_mutex_unlock(mutex);
	return true;
}

static void log_locks(obs_source_t *source)
{
	struct obs_core *core = &current->core;

	log_str(mutex_free(&core->data.audio_sources_mutex) ? " sources free" : " sources held");
	log_str(mutex_free(&core->video.mixes_mutex) ? " mixes free" : " mixes held");
	log_str(mutex_free(&core->audio.task_mutex) ? " tasks free" : " tasks held");
	if (source)
		log_str(mutex_free(&source->audio_buf_mutex) ? " buf free" : " buf held");
}

uint32_t oracle_tick_sample_rate(const audio_t *audio)
{
	log_str(audio == (const audio_t *)current ? "rate;" : "rate other;");
	return current->rate;
}

size_t oracle_tick_channels(const audio_t *audio)
{
	log_str(audio == (const audio_t *)current ? "channels;" : "channels other;");
	return current->channels;
}

obs_source_t *oracle_tick_get_ref(obs_source_t *source)
{
	struct tick_src *s = src_of(source);

	log_src("ref", source);
	if (s->dead)
		return NULL;
	s->refs++;
	return source;
}

void oracle_tick_release(obs_source_t *source)
{
	log_src("release", source);
	if (source)
		src_of(source)->refs--;
}

const char *oracle_tick_get_name(const obs_source_t *source)
{
	return src_of(source)->name;
}

bool oracle_tick_removed(const obs_source_t *source)
{
	log_src("removed", source);
	return src_of(source)->removed;
}

bool oracle_tick_active(const obs_source_t *source)
{
	log_src("active", source);
	return src_of(source)->active;
}

/* Writes each mix the source and `mixers` share with values of its own,
 * for the first `channels` channels with a buffer. */
void oracle_tick_audio_render(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate, size_t size)
{
	struct tick_src *s = src_of(source);

	log_src("render", source);
	log_hex(mixers);
	log_str(" ");
	log_hex(channels);
	log_str(" ");
	log_hex(sample_rate);
	log_str(" ");
	log_hex(size);
	log_locks(source);
	log_str(";");

	s->renders++;
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		if (!(mixers & source->audio_mixers & (1u << mix)))
			continue;
		for (size_t ch = 0; ch < channels && ch < MAX_AUDIO_CHANNELS; ch++) {
			float *buf = source->audio_output_buf[mix][ch];
			if (!buf)
				continue;
			float base = (float)(s->id * 1000) + (float)(s->renders * 10) + (float)(mix * 2 + ch);
			for (size_t i = 0; i < AUDIO_OUTPUT_FRAMES; i++)
				buf[i] = base + (float)(i % 97) * 0.25f;
		}
	}
}

/* Calls the callback on each source of the source's tree, in order. */
void oracle_tick_enum_active_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	struct tick_src *s = src_of(source);

	log_src("tree", source);
	for (size_t i = 0; i < s->tree_len; i++)
		enum_callback(source, current->src[s->tree[i]], param);
}

/* The format with each value in hex: %s, %d, the %llu and PRIu64 forms and
 * %.02f (as its bits) are all its callers use. */
void oracle_tick_blog(int log_level, const char *format, ...)
{
	va_list args;

	va_start(args, format);
	log_str("blog ");
	log_hex((uint64_t)(int64_t)log_level);
	log_str(" ");
	for (const char *p = format; *p; p++) {
		if (*p != '%') {
			char one[2] = {*p, 0};
			log_str(one);
			continue;
		}
		int longs = 0;
		while (p[1] && p[1] != 's' && p[1] != 'u' && p[1] != 'd' && p[1] != 'f') {
			if (p[1] == 'l')
				longs++;
			p++;
		}
		p++;
		if (*p == 's') {
			const char *text = va_arg(args, const char *);
			log_str(text ? text : "(null)");
		} else if (*p == 'd') {
			log_hex((uint64_t)(int64_t)va_arg(args, int));
		} else if (*p == 'f') {
			double value = va_arg(args, double);
			uint64_t bits;
			memcpy(&bits, &value, sizeof(bits));
			log_hex(bits);
		} else if (*p == 'u') {
			log_hex(longs ? (uint64_t)va_arg(args, uint64_t) : (uint64_t)va_arg(args, unsigned));
		} else {
			break;
		}
	}
	va_end(args);
	log_str(";");
}

static void tick_task(void *param)
{
	struct tick_task *task = param;
	struct oracle_tick_world *world = task->world;

	log_str("task ");
	log_hex((uint64_t)(int64_t)task->id);
	log_str(" out_ts ");
	log_hex(world->out_ts);
	log_locks(NULL);
	log_str(";");

	if (task->chain > 0 && world->task_len < ORACLE_TICK_TASKS) {
		struct tick_task *next = &world->tasks[world->task_len++];
		struct obs_task_info info = {tick_task, next};

		next->world = world;
		next->id = task->id + 100;
		next->chain = task->chain - 1;
		pthread_mutex_lock(&world->core.audio.task_mutex);
		deque_push_back(&world->core.audio.tasks, &info, sizeof(info));
		pthread_mutex_unlock(&world->core.audio.task_mutex);
	}
}

void *oracle_tick_new(uint32_t rate, size_t channels, const struct oracle_tick_core_init *init)
{
	struct oracle_tick_world *world = bzalloc(sizeof(*world));
	struct obs_core *core = &world->core;

	world->rate = rate;
	world->channels = channels;
	/* never dereferenced: the rate and channel stand-ins answer for it */
	core->audio.audio = (audio_t *)world;
	pthread_mutex_init(&core->audio.task_mutex, NULL);
	pthread_mutex_init(&core->data.audio_sources_mutex, NULL);
	pthread_mutex_init(&core->video.mixes_mutex, NULL);
	core->audio.buffered_ts = init->buffered_ts;
	core->audio.buffering_wait_ticks = init->buffering_wait_ticks;
	core->audio.total_buffering_ticks = init->total_buffering_ticks;
	core->audio.max_buffering_ticks = init->max_buffering_ticks;
	core->audio.fixed_buffer = init->fixed_buffer;
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			world->out[mix].data[ch] = bzalloc(AUDIO_OUTPUT_FRAMES * sizeof(float));
	return world;
}

void oracle_tick_free(void *opaque)
{
	struct oracle_tick_world *world = opaque;
	struct obs_core *core = &world->core;

	for (size_t i = 0; i < world->src_len; i++) {
		obs_source_t *source = world->src[i];
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			deque_free(&source->audio_input_buf[ch]);
		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				bfree(source->audio_output_buf[mix][ch]);
		pthread_mutex_destroy(&source->audio_buf_mutex);
		bfree(source->context.data);
		bfree(source);
	}
	for (size_t i = 0; i < world->view_len; i++)
		pthread_mutex_destroy(&world->views[i].channels_mutex);
	for (size_t i = 0; i < core->video.mixes.num; i++)
		bfree(core->video.mixes.array[i]);
	da_free(core->video.mixes);
	da_free(core->audio.render_order);
	da_free(core->audio.root_nodes);
	deque_free(&core->audio.buffered_timestamps);
	deque_free(&core->audio.tasks);
	pthread_mutex_destroy(&core->audio.task_mutex);
	pthread_mutex_destroy(&core->data.audio_sources_mutex);
	pthread_mutex_destroy(&core->video.mixes_mutex);
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			bfree(world->out[mix].data[ch]);
	if (current == world)
		current = NULL;
	bfree(world);
}

static void push_input(obs_source_t *source, size_t channels, uint32_t frames)
{
	struct tick_src *s = src_of(source);

	for (size_t ch = 0; ch < channels && ch < MAX_AUDIO_CHANNELS; ch++) {
		for (uint32_t i = 0; i < frames; i++) {
			float value = (float)(s->id * 100 + (int)ch) + (float)(i % 53) * 0.5f;
			deque_push_back(&source->audio_input_buf[ch], &value, sizeof(value));
		}
	}
}

int oracle_tick_add_source(void *opaque, const struct oracle_tick_source_init *init)
{
	struct oracle_tick_world *world = opaque;
	obs_source_t *source = bzalloc(sizeof(*source));
	struct tick_src *s = bzalloc(sizeof(*s));
	int id = (int)world->src_len;

	s->id = id;
	s->name[0] = 's';
	s->name[1] = (char)('a' + id);
	s->removed = init->removed;
	s->active = init->active;
	s->dead = init->dead;
	source->context.data = s;
	pthread_mutex_init(&source->audio_buf_mutex, NULL);
	source->info.type = (enum obs_source_type)init->kind;
	source->info.output_flags = init->output_flags;
	if (init->audio_render)
		source->info.audio_render = dummy_audio_render;
	source->audio_pending = init->pending;
	source->pending_stop = init->pending_stop;
	source->timing_set = init->timing_set;
	source->audio_is_duplicated = init->duplicated;
	source->muted = init->muted;
	source->monitoring_enabled = init->monitoring;
	source->audio_ts = init->audio_ts;
	source->audio_mixers = init->audio_mixers;
	source->volume = init->volume;
	source->last_audio_input_buf_size = 7;
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		if (init->out_mixes & (1u << mix))
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				source->audio_output_buf[mix][ch] = bzalloc(AUDIO_OUTPUT_FRAMES * sizeof(float));
	world->src[world->src_len++] = source;
	push_input(source, world->channels, init->frames);
	return id;
}

void oracle_tick_set_tree(void *opaque, int source, const int *tree, size_t len)
{
	struct oracle_tick_world *world = opaque;
	struct tick_src *s = src_of(world->src[source]);

	s->tree_len = len < ORACLE_TICK_SOURCES ? len : ORACLE_TICK_SOURCES;
	memcpy(s->tree, tree, s->tree_len * sizeof(*tree));
}

void oracle_tick_link_audio(void *opaque, const int *order, size_t len)
{
	struct oracle_tick_world *world = opaque;

	world->core.data.first_audio_source = len ? world->src[order[0]] : NULL;
	for (size_t i = 0; i < len; i++)
		world->src[order[i]]->next_audio_source = i + 1 < len ? world->src[order[i + 1]] : NULL;
}

int oracle_tick_add_view(void *opaque)
{
	struct oracle_tick_world *world = opaque;
	int view = (int)world->view_len++;

	pthread_mutex_init(&world->views[view].channels_mutex, NULL);
	return view;
}

void oracle_tick_set_channel(void *opaque, int view, uint32_t channel, int source)
{
	struct oracle_tick_world *world = opaque;

	world->views[view].channels[channel] = source < 0 ? NULL : world->src[source];
}

void oracle_tick_add_mix(void *opaque, int view, bool mix_audio)
{
	struct oracle_tick_world *world = opaque;
	struct obs_core_video_mix *mix = bzalloc(sizeof(*mix));

	mix->view = view < 0 ? NULL : &world->views[view];
	mix->mix_audio = mix_audio;
	da_push_back(world->core.video.mixes, &mix);
}

void oracle_tick_set_dedup(void *opaque, int source)
{
	struct oracle_tick_world *world = opaque;

	world->core.audio.monitoring_duplicating_source = source < 0 ? NULL : world->src[source];
}

void oracle_tick_queue_task(void *opaque, int id, int chain)
{
	struct oracle_tick_world *world = opaque;

	if (world->task_len >= ORACLE_TICK_TASKS)
		return;
	struct tick_task *task = &world->tasks[world->task_len++];
	struct obs_task_info info = {tick_task, task};

	task->world = world;
	task->id = id;
	task->chain = chain;
	deque_push_back(&world->core.audio.tasks, &info, sizeof(info));
}

/* New audio for a source between ticks: `frames` more frames in each
 * channel, and `audio_ts` as its timestamp when it has none. */
void oracle_tick_feed(void *opaque, int source, uint32_t frames, uint64_t audio_ts)
{
	struct oracle_tick_world *world = opaque;
	obs_source_t *s = world->src[source];

	push_input(s, world->channels, frames);
	if (!s->audio_ts)
		s->audio_ts = audio_ts;
}

void oracle_tick_use(void *opaque)
{
	current = opaque;
	oracle_tick_obs = &current->core;
}

/* The audio output clears the mixes before each tick. The *out_ts slot is
 * set to a value no tick has, so a task shows whether it was written. */
struct audio_output_data *oracle_tick_clear_mixes(void *opaque)
{
	struct oracle_tick_world *world = opaque;

	world->out_ts = 0x5a5a5a5a5a5a5a5aULL;
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			memset(world->out[mix].data[ch], 0, AUDIO_OUTPUT_FRAMES * sizeof(float));
	return world->out;
}

/* The world's *out_ts slot. */
uint64_t *oracle_tick_out_ts(void *opaque)
{
	struct oracle_tick_world *world = opaque;

	return &world->out_ts;
}

bool oracle_tick_run_c(void *opaque, uint64_t start, uint64_t end, uint32_t mixers, uint64_t *out_ts)
{
	struct oracle_tick_world *world = opaque;
	struct audio_output_data *mixes = oracle_tick_clear_mixes(world);

	oracle_tick_use(world);
	bool result = oracle_audio_callback(NULL, start, end, &world->out_ts, mixers, mixes);
	*out_ts = world->out_ts;
	return result;
}

static void put_ids(char *out, size_t cap, size_t *len, obs_source_t **array, size_t num)
{
	for (size_t i = 0; i < num; i++) {
		put_str(out, cap, len, " ");
		put_hex(out, cap, len, (uint64_t)(int64_t)id_of(array[i]));
	}
}

/* The log, the core's buffering fields and lists, every source's fields,
 * input and output, and the mixes. */
size_t oracle_tick_state(void *opaque, char *out, size_t cap)
{
	struct oracle_tick_world *world = opaque;
	struct obs_core_audio *audio = &world->core.audio;
	size_t len = 0;

	out[0] = 0;
	put_str(out, cap, &len, world->log);
	put_str(out, cap, &len, "|bts ");
	put_hex(out, cap, &len, audio->buffered_ts);
	put_str(out, cap, &len, " wait ");
	put_hex(out, cap, &len, audio->buffering_wait_ticks);
	put_str(out, cap, &len, " total ");
	put_hex(out, cap, &len, (uint64_t)(int64_t)audio->total_buffering_ticks);
	put_str(out, cap, &len, " stamps");
	{
		struct deque copy = audio->buffered_timestamps;
		size_t n = copy.size / sizeof(struct ts_info);
		for (size_t i = 0; i < n; i++) {
			struct ts_info ts;
			deque_pop_front(&copy, &ts, sizeof(ts));
			put_str(out, cap, &len, " ");
			put_hex(out, cap, &len, ts.start);
			put_str(out, cap, &len, "-");
			put_hex(out, cap, &len, ts.end);
		}
	}
	put_str(out, cap, &len, " order");
	put_ids(out, cap, &len, audio->render_order.array, audio->render_order.num);
	put_str(out, cap, &len, " roots");
	put_ids(out, cap, &len, audio->root_nodes.array, audio->root_nodes.num);
	put_str(out, cap, &len, " tasks ");
	put_hex(out, cap, &len, audio->tasks.size);
	put_str(out, cap, &len, mutex_free(&audio->task_mutex) ? " free" : " held");
	put_str(out, cap, &len, mutex_free(&world->core.data.audio_sources_mutex) ? " free" : " held");
	put_str(out, cap, &len, mutex_free(&world->core.video.mixes_mutex) ? " free" : " held");

	for (size_t i = 0; i < world->src_len; i++) {
		obs_source_t *s = world->src[i];
		uint64_t hash = 0xcbf29ce484222325ULL;

		put_str(out, cap, &len, "|src ");
		put_hex(out, cap, &len, i);
		put_str(out, cap, &len, " refs ");
		put_hex(out, cap, &len, (uint64_t)(int64_t)src_of(s)->refs);
		put_str(out, cap, &len, s->audio_pending ? " pending" : " ready");
		put_str(out, cap, &len, s->pending_stop ? " stop" : " nostop");
		put_str(out, cap, &len, s->timing_set ? " timed" : " untimed");
		put_str(out, cap, &len, s->audio_is_duplicated ? " dup" : " single");
		put_str(out, cap, &len, " ts ");
		put_hex(out, cap, &len, s->audio_ts);
		put_str(out, cap, &len, " last ");
		put_hex(out, cap, &len, s->last_audio_input_buf_size);
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++) {
			size_t size = s->audio_input_buf[ch].size;
			put_str(out, cap, &len, " in ");
			put_hex(out, cap, &len, size);
			if (size) {
				uint8_t *bytes = bmalloc(size);
				struct deque copy = s->audio_input_buf[ch];
				deque_peek_front(&copy, bytes, size);
				hash = fnv_add(hash, bytes, size);
				bfree(bytes);
			}
		}
		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				if (s->audio_output_buf[mix][ch])
					hash = fnv_add(hash, s->audio_output_buf[mix][ch],
						       AUDIO_OUTPUT_FRAMES * sizeof(float));
		put_str(out, cap, &len, " hash ");
		put_hex(out, cap, &len, hash);
		put_str(out, cap, &len, mutex_free(&s->audio_buf_mutex) ? " free" : " held");
	}

	put_str(out, cap, &len, "|mixes");
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		uint64_t hash = 0xcbf29ce484222325ULL;
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			hash = fnv_add(hash, world->out[mix].data[ch], AUDIO_OUTPUT_FRAMES * sizeof(float));
		put_str(out, cap, &len, " ");
		put_hex(out, cap, &len, hash);
	}
	return len;
}

/* The layouts the shim's #[repr(C)] types copy, measured from the real
 * headers: struct obs_task_info (obs-internal.h) and one row of a source's
 * audio_output_buf (obs_audio_seam_output_row_t, obs-audio-seam.h). */
size_t oracle_tick_task_info_size(void);
size_t oracle_tick_task_info_align(void);
size_t oracle_tick_task_info_offset_task(void);
size_t oracle_tick_task_info_offset_param(void);
size_t oracle_tick_output_row_size(void);
size_t oracle_tick_output_row_align(void);

size_t oracle_tick_task_info_size(void)
{
	return sizeof(struct obs_task_info);
}

size_t oracle_tick_task_info_align(void)
{
	return _Alignof(struct obs_task_info);
}

size_t oracle_tick_task_info_offset_task(void)
{
	return offsetof(struct obs_task_info, task);
}

size_t oracle_tick_task_info_offset_param(void)
{
	return offsetof(struct obs_task_info, param);
}

size_t oracle_tick_output_row_size(void)
{
	return sizeof(((struct obs_source *)NULL)->audio_output_buf[0]);
}

size_t oracle_tick_output_row_align(void)
{
	return _Alignof(obs_audio_seam_output_row_t);
}
