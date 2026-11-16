/* Test-only oracle: the original libobs/obs-source-deinterlace.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims (rust/obs-source-seam/src/ffi/source_deinterlace.rs).
 *
 * The select, pacing and draw decisions (obs_source_deinterlace_select_*,
 * _pacing_*, _draw_*) are the Rust ones, as in the hybrid libobs; the C
 * reads the fields and takes async_mutex directly, the Rust shims through
 * the deinterlace seam, compiled under its own names by
 * oracle/source_deinterlace_links.c. Both read the same core,
 * oracle_deinterlace_obs, which points at this file's own core (its frame
 * interval, its video stats and its eight effect slots). The C's other
 * calls go to recording stand-ins below, which the parity test hands the
 * Rust shims too (their Calls): the frame cache (remove_async_frame(),
 * obs_source_release_frame()), the async texture upload
 * (set_async_texture_size(), update_async_textures()), the deinterlace
 * texture size, obs_load_effect() (which fills an empty slot with a fake
 * effect, unless a test makes it fail), the graphics context, the SDR white
 * level and every graphics call, on fake textures, texrenders, effects and
 * parameters, each a byte of a static array, whose answers a test sets.
 *
 * Each stand-in appends one event to a log, in order: what it was called
 * with (frames, textures, texrenders, effects and parameters as pointers,
 * names as strings, the floats bit for bit; a frame back to the cache with
 * the queue's length then, and a frame's references), whether the watched source's
 * async_mutex was held, and how deep the graphics context was entered.
 *
 * A test source is a zeroed struct obs_source with a default (not
 * recursive) async_mutex, so the stand-ins can tell whether it is held, and
 * an async_frames queue of test frames. */

#include "libobs/build/libobs-ported-source-deinterlace.pch.h"

void oracle_deinterlace_process_last_frame(obs_source_t *source, uint64_t sys_time);
void oracle_deinterlace_update_async_video(obs_source_t *source);
void oracle_deinterlace_render(obs_source_t *s);
void oracle_obs_source_set_deinterlace_mode(obs_source_t *source, enum obs_deinterlace_mode mode);
void oracle_obs_source_set_deinterlace_field_order(obs_source_t *source, enum obs_deinterlace_field_order field_order);

void oracle_deint_remove_async_frame(obs_source_t *source, struct obs_source_frame *frame);
bool oracle_deint_set_async_texture_size(struct obs_source *source, const struct obs_source_frame *frame);
bool oracle_deint_update_async_textures(struct obs_source *source, const struct obs_source_frame *frame,
					gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender);
void oracle_deint_release_frame(obs_source_t *source, struct obs_source_frame *frame);
void oracle_deint_set_deinterlace_texture_size(obs_source_t *source);
gs_effect_t *oracle_deint_load_effect(gs_effect_t **effect, const char *file);
void oracle_deint_enter_graphics(void);
void oracle_deint_leave_graphics(void);
float oracle_deint_sdr_white_level(void);
gs_eparam_t *oracle_deint_get_param_by_name(const gs_effect_t *effect, const char *name);
gs_texture_t *oracle_deint_texrender_get_texture(const gs_texrender_t *texrender);
bool oracle_deint_get_linear_srgb(void);
enum gs_color_space oracle_deint_get_color_space(void);
bool oracle_deint_framebuffer_srgb_enabled(void);
void oracle_deint_enable_framebuffer_srgb(bool enable);
void oracle_deint_set_texture_srgb(gs_eparam_t *param, gs_texture_t *val);
void oracle_deint_set_texture(gs_eparam_t *param, gs_texture_t *val);
void oracle_deint_set_float(gs_eparam_t *param, float val);
void oracle_deint_set_int(gs_eparam_t *param, int val);
void oracle_deint_set_vec2(gs_eparam_t *param, const struct vec2 *val);
void oracle_deint_set_bool(gs_eparam_t *param, bool val);
bool oracle_deint_effect_loop(gs_effect_t *effect, const char *name);
void oracle_deint_draw_sprite(gs_texture_t *tex, uint32_t flip, uint32_t width, uint32_t height);
void oracle_deint_texture_destroy(gs_texture_t *tex);
void oracle_deint_texrender_destroy(gs_texrender_t *texrender);

extern struct obs_core *oracle_deinterlace_obs;

#define deinterlace_process_last_frame oracle_deinterlace_process_last_frame
#define deinterlace_update_async_video oracle_deinterlace_update_async_video
#define deinterlace_render oracle_deinterlace_render
#define obs_source_set_deinterlace_mode oracle_obs_source_set_deinterlace_mode
#define obs_source_set_deinterlace_field_order oracle_obs_source_set_deinterlace_field_order
#define remove_async_frame oracle_deint_remove_async_frame
#define set_async_texture_size oracle_deint_set_async_texture_size
#define update_async_textures oracle_deint_update_async_textures
#define obs_source_release_frame oracle_deint_release_frame
#define set_deinterlace_texture_size oracle_deint_set_deinterlace_texture_size
#define obs_load_effect oracle_deint_load_effect
#define obs_enter_graphics oracle_deint_enter_graphics
#define obs_leave_graphics oracle_deint_leave_graphics
#define obs_get_video_sdr_white_level oracle_deint_sdr_white_level
#define gs_effect_get_param_by_name oracle_deint_get_param_by_name
#define gs_texrender_get_texture oracle_deint_texrender_get_texture
#define gs_get_linear_srgb oracle_deint_get_linear_srgb
#define gs_get_color_space oracle_deint_get_color_space
#define gs_framebuffer_srgb_enabled oracle_deint_framebuffer_srgb_enabled
#define gs_enable_framebuffer_srgb oracle_deint_enable_framebuffer_srgb
#define gs_effect_set_texture_srgb oracle_deint_set_texture_srgb
#define gs_effect_set_texture oracle_deint_set_texture
#define gs_effect_set_float oracle_deint_set_float
#define gs_effect_set_int oracle_deint_set_int
#define gs_effect_set_vec2 oracle_deint_set_vec2
#define gs_effect_set_bool oracle_deint_set_bool
#define gs_effect_loop oracle_deint_effect_loop
#define gs_draw_sprite oracle_deint_draw_sprite
#define gs_texture_destroy oracle_deint_texture_destroy
#define gs_texrender_destroy oracle_deint_texrender_destroy
#define obs oracle_deinterlace_obs

/* libobs builds the file without the oracle's -Wmissing-braces (struct
 * vec2 is a union) and its conversion warnings. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wmissing-braces"
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wconversion"
#endif

#include "obs-source-deinterlace.c.h"

#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

#undef obs
#undef gs_texrender_destroy
#undef gs_texture_destroy
#undef gs_draw_sprite
#undef gs_effect_loop
#undef gs_effect_set_bool
#undef gs_effect_set_vec2
#undef gs_effect_set_int
#undef gs_effect_set_float
#undef gs_effect_set_texture
#undef gs_effect_set_texture_srgb
#undef gs_enable_framebuffer_srgb
#undef gs_framebuffer_srgb_enabled
#undef gs_get_color_space
#undef gs_get_linear_srgb
#undef gs_texrender_get_texture
#undef gs_effect_get_param_by_name
#undef obs_get_video_sdr_white_level
#undef obs_leave_graphics
#undef obs_enter_graphics
#undef obs_load_effect
#undef set_deinterlace_texture_size
#undef obs_source_release_frame
#undef update_async_textures
#undef set_async_texture_size
#undef remove_async_frame
#undef obs_source_set_deinterlace_field_order
#undef obs_source_set_deinterlace_mode
#undef deinterlace_render
#undef deinterlace_update_async_video
#undef deinterlace_process_last_frame

/* The event kinds. */
enum {
	EV_REMOVE = 1,
	EV_TEXTURE_SIZE,
	EV_UPDATE_TEXTURES,
	EV_RELEASE,
	EV_DEINTERLACE_SIZE,
	EV_LOAD_EFFECT,
	EV_ENTER,
	EV_LEAVE,
	EV_WHITE_LEVEL,
	EV_PARAM,
	EV_TEXRENDER_TEXTURE,
	EV_LINEAR_SRGB,
	EV_COLOR_SPACE,
	EV_FRAMEBUFFER_SRGB,
	EV_ENABLE_FRAMEBUFFER_SRGB,
	EV_SET_TEXTURE_SRGB,
	EV_SET_TEXTURE,
	EV_SET_FLOAT,
	EV_SET_INT,
	EV_SET_VEC2,
	EV_SET_BOOL,
	EV_EFFECT_LOOP,
	EV_DRAW_SPRITE,
	EV_TEXTURE_DESTROY,
	EV_TEXRENDER_DESTROY,
};

#define ORACLE_DEINT_EVENTS 512
#define ORACLE_DEINT_HANDLES 32
#define ORACLE_DEINT_PARAMS 6

/* One call: its kind, up to two pointers, a name, up to three ints, up to
 * two floats (as bits), whether the watched source's async_mutex was held
 * (-1 when none is watched) and the graphics depth. */
struct oracle_deint_event {
	int kind;
	const void *p[2];
	const char *name;
	int i[3];
	uint32_t f[2];
	int held;
	int graphics;
};

/* What the stand-ins answer. */
struct oracle_deint_answers {
	bool texture_size_ok;
	bool load_ok;
	bool params_found;
	bool texrender_texture;
	bool linear_srgb;
	bool framebuffer_srgb;
	int color_space;
	int loops;
	float white_level;
};

/* The fields of a test source the deinterlace reads or writes, besides the
 * queue. */
struct oracle_deint_state {
	bool unbuffered;
	uint64_t last_sys_timestamp;
	uint64_t last_frame_ts;
	uint64_t offset;
	uint64_t frame_ts;
	uint32_t half_duration;
	struct obs_source_frame *cur;
	struct obs_source_frame *prev;
	int format;
	int trc;
	uint32_t width;
	uint32_t height;
	bool flip;
	gs_texture_t *textures[MAX_AV_PLANES];
	gs_texture_t *prev_textures[MAX_AV_PLANES];
	gs_texrender_t *texrender;
	gs_texrender_t *prev_texrender;
	bool rendered;
	gs_effect_t *effect;
	int mode;
	bool top_first;
};

void oracle_deint_reset(obs_source_t *watched, const struct oracle_deint_answers *answers, uint64_t interval,
			uint64_t video_time);
size_t oracle_deint_logged(void);
const struct oracle_deint_event *oracle_deint_event(size_t i);
gs_effect_t *oracle_deint_core_effect(size_t idx);
gs_texture_t *oracle_deint_texture(size_t n);
gs_texrender_t *oracle_deint_texrender(size_t n);
gs_effect_t *oracle_deint_effect(size_t n);
gs_eparam_t *oracle_deint_param(size_t n);
obs_source_t *oracle_deint_source_new(void);
void oracle_deint_source_free(obs_source_t *source);
void oracle_deint_push(obs_source_t *source, struct obs_source_frame *frame);
size_t oracle_deint_queued(const obs_source_t *source);
struct obs_source_frame *oracle_deint_queue(const obs_source_t *source, size_t idx);
void oracle_deint_set_state(obs_source_t *source, const struct oracle_deint_state *state);
void oracle_deint_get_state(const obs_source_t *source, struct oracle_deint_state *state);
struct obs_source_frame *oracle_deint_frame_new(uint64_t timestamp, bool prev_frame);
void oracle_deint_frame_free(struct obs_source_frame *frame);
long oracle_deint_frame_refs(const struct obs_source_frame *frame);
void oracle_deint_lock(obs_source_t *source);
void oracle_deint_unlock(obs_source_t *source);

static struct obs_core core;
static bool core_ready;

static unsigned char textures[ORACLE_DEINT_HANDLES];
static unsigned char texrenders[ORACLE_DEINT_HANDLES];
static unsigned char effects[ORACLE_DEINT_HANDLES];
static unsigned char params[ORACLE_DEINT_PARAMS];

static const char *const param_names[ORACLE_DEINT_PARAMS] = {
	"image", "previous_image", "multiplier", "field_order", "frame2", "dimensions",
};

static const char *const effect_files[8] = {
	"deinterlace_discard.effect",  "deinterlace_discard_2x.effect", "deinterlace_blend.effect",
	"deinterlace_blend_2x.effect", "deinterlace_linear.effect",     "deinterlace_linear_2x.effect",
	"deinterlace_yadif.effect",    "deinterlace_yadif_2x.effect",
};

static struct {
	obs_source_t *watched;
	struct oracle_deint_answers answers;
	int graphics;
	struct oracle_deint_event log[ORACLE_DEINT_EVENTS];
	size_t logged;
} st;

/* Watches source, sets what the stand-ins give, and points the core (the
 * frame interval, the video time, and the eight effect slots emptied) at
 * this file's own. Forgets the log. */
void oracle_deint_reset(obs_source_t *watched, const struct oracle_deint_answers *answers, uint64_t interval,
			uint64_t video_time)
{
	memset(&st, 0, sizeof(st));
	st.watched = watched;
	st.answers = *answers;

	if (!core_ready) {
		pthread_mutex_init(&core.video.stats.mutex, NULL);
		core_ready = true;
	}
	core.video.video_frame_interval_ns = interval;
	core.video.stats.frame_time_ns = video_time;
	core.video.deinterlace_discard_effect = NULL;
	core.video.deinterlace_discard_2x_effect = NULL;
	core.video.deinterlace_blend_effect = NULL;
	core.video.deinterlace_blend_2x_effect = NULL;
	core.video.deinterlace_linear_effect = NULL;
	core.video.deinterlace_linear_2x_effect = NULL;
	core.video.deinterlace_yadif_effect = NULL;
	core.video.deinterlace_yadif_2x_effect = NULL;
	oracle_deinterlace_obs = &core;
}

size_t oracle_deint_logged(void)
{
	return st.logged;
}

const struct oracle_deint_event *oracle_deint_event(size_t i)
{
	return i < st.logged ? &st.log[i] : NULL;
}

/* The core's effect slot idx, 0 discard to 7 yadif 2x. */
gs_effect_t *oracle_deint_core_effect(size_t idx)
{
	gs_effect_t *const slots[8] = {
		core.video.deinterlace_discard_effect, core.video.deinterlace_discard_2x_effect,
		core.video.deinterlace_blend_effect,   core.video.deinterlace_blend_2x_effect,
		core.video.deinterlace_linear_effect,  core.video.deinterlace_linear_2x_effect,
		core.video.deinterlace_yadif_effect,   core.video.deinterlace_yadif_2x_effect,
	};
	return idx < 8 ? slots[idx] : NULL;
}

/* Fake handle n of each kind: the address of byte n of a static array.
 * Never NULL. */
gs_texture_t *oracle_deint_texture(size_t n)
{
	return (gs_texture_t *)&textures[n % ORACLE_DEINT_HANDLES];
}

gs_texrender_t *oracle_deint_texrender(size_t n)
{
	return (gs_texrender_t *)&texrenders[n % ORACLE_DEINT_HANDLES];
}

gs_effect_t *oracle_deint_effect(size_t n)
{
	return (gs_effect_t *)&effects[n % ORACLE_DEINT_HANDLES];
}

gs_eparam_t *oracle_deint_param(size_t n)
{
	return (gs_eparam_t *)&params[n % ORACLE_DEINT_PARAMS];
}

obs_source_t *oracle_deint_source_new(void)
{
	obs_source_t *source = bzalloc(sizeof(*source));
	pthread_mutex_init(&source->async_mutex, NULL);
	da_init(source->async_frames);
	return source;
}

void oracle_deint_source_free(obs_source_t *source)
{
	da_free(source->async_frames);
	pthread_mutex_destroy(&source->async_mutex);
	bfree(source);
}

void oracle_deint_push(obs_source_t *source, struct obs_source_frame *frame)
{
	da_push_back(source->async_frames, &frame);
}

size_t oracle_deint_queued(const obs_source_t *source)
{
	return source->async_frames.num;
}

struct obs_source_frame *oracle_deint_queue(const obs_source_t *source, size_t idx)
{
	return idx < source->async_frames.num ? source->async_frames.array[idx] : NULL;
}

void oracle_deint_set_state(obs_source_t *source, const struct oracle_deint_state *state)
{
	source->async_unbuffered = state->unbuffered;
	source->last_sys_timestamp = state->last_sys_timestamp;
	source->last_frame_ts = state->last_frame_ts;
	source->deinterlace_offset = state->offset;
	source->deinterlace_frame_ts = state->frame_ts;
	source->deinterlace_half_duration = state->half_duration;
	source->cur_async_frame = state->cur;
	source->prev_async_frame = state->prev;
	source->async_format = (enum video_format)state->format;
	source->async_trc = (uint8_t)state->trc;
	source->async_width = state->width;
	source->async_height = state->height;
	source->async_flip = state->flip;
	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		source->async_textures[c] = state->textures[c];
		source->async_prev_textures[c] = state->prev_textures[c];
	}
	source->async_texrender = state->texrender;
	source->async_prev_texrender = state->prev_texrender;
	source->deinterlace_rendered = state->rendered;
	source->deinterlace_effect = state->effect;
	source->deinterlace_mode = (enum obs_deinterlace_mode)state->mode;
	source->deinterlace_top_first = state->top_first;
}

void oracle_deint_get_state(const obs_source_t *source, struct oracle_deint_state *state)
{
	state->unbuffered = source->async_unbuffered;
	state->last_sys_timestamp = source->last_sys_timestamp;
	state->last_frame_ts = source->last_frame_ts;
	state->offset = source->deinterlace_offset;
	state->frame_ts = source->deinterlace_frame_ts;
	state->half_duration = source->deinterlace_half_duration;
	state->cur = source->cur_async_frame;
	state->prev = source->prev_async_frame;
	state->format = (int)source->async_format;
	state->trc = (int)source->async_trc;
	state->width = source->async_width;
	state->height = source->async_height;
	state->flip = source->async_flip;
	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		state->textures[c] = source->async_textures[c];
		state->prev_textures[c] = source->async_prev_textures[c];
	}
	state->texrender = source->async_texrender;
	state->prev_texrender = source->async_prev_texrender;
	state->rendered = source->deinterlace_rendered;
	state->effect = source->deinterlace_effect;
	state->mode = (int)source->deinterlace_mode;
	state->top_first = source->deinterlace_top_first;
}

struct obs_source_frame *oracle_deint_frame_new(uint64_t timestamp, bool prev_frame)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->timestamp = timestamp;
	frame->prev_frame = prev_frame;
	frame->refs = 1;
	return frame;
}

void oracle_deint_frame_free(struct obs_source_frame *frame)
{
	bfree(frame);
}

long oracle_deint_frame_refs(const struct obs_source_frame *frame)
{
	return frame->refs;
}

/* What async_tick() holds around deinterlace_process_last_frame(). */
void oracle_deint_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->async_mutex);
}

void oracle_deint_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->async_mutex);
}

static int held(void)
{
	if (!st.watched)
		return -1;
	if (pthread_mutex_trylock(&st.watched->async_mutex) != 0)
		return 1;
	pthread_mutex_unlock(&st.watched->async_mutex);
	return 0;
}

static struct oracle_deint_event *record(int kind)
{
	static struct oracle_deint_event overflow;
	struct oracle_deint_event *e = st.logged < ORACLE_DEINT_EVENTS ? &st.log[st.logged++] : &overflow;

	memset(e, 0, sizeof(*e));
	e->kind = kind;
	e->held = held();
	e->graphics = st.graphics;
	return e;
}

static uint32_t bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

void oracle_deint_remove_async_frame(obs_source_t *source, struct obs_source_frame *frame)
{
	struct oracle_deint_event *e = record(EV_REMOVE);
	e->p[0] = source;
	e->p[1] = frame;
	e->i[0] = (int)source->async_frames.num;
}

bool oracle_deint_set_async_texture_size(struct obs_source *source, const struct obs_source_frame *frame)
{
	struct oracle_deint_event *e = record(EV_TEXTURE_SIZE);
	e->p[0] = source;
	e->p[1] = frame;
	e->i[0] = (int)frame->refs;
	return st.answers.texture_size_ok;
}

bool oracle_deint_update_async_textures(struct obs_source *source, const struct obs_source_frame *frame,
					gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender)
{
	struct oracle_deint_event *e = record(EV_UPDATE_TEXTURES);
	e->p[0] = frame;
	e->p[1] = texrender;
	e->i[0] = tex == source->async_prev_textures;
	e->i[1] = tex == source->async_textures;
	e->i[2] = (int)frame->refs;
	return true;
}

void oracle_deint_release_frame(obs_source_t *source, struct obs_source_frame *frame)
{
	struct oracle_deint_event *e = record(EV_RELEASE);
	e->p[0] = source;
	e->p[1] = frame;
	e->i[0] = (int)frame->refs;
}

void oracle_deint_set_deinterlace_texture_size(obs_source_t *source)
{
	struct oracle_deint_event *e = record(EV_DEINTERLACE_SIZE);
	e->p[0] = source;
}

/* obs_load_effect(): an empty slot gets its fake effect (one per file),
 * unless the load fails; the slot's effect comes back. */
gs_effect_t *oracle_deint_load_effect(gs_effect_t **effect, const char *file)
{
	struct oracle_deint_event *e = record(EV_LOAD_EFFECT);
	e->p[0] = effect;
	e->name = file;
	if (!*effect && st.answers.load_ok) {
		for (size_t i = 0; i < 8; i++) {
			if (strcmp(file, effect_files[i]) == 0)
				*effect = oracle_deint_effect(16 + i);
		}
	}
	return *effect;
}

void oracle_deint_enter_graphics(void)
{
	record(EV_ENTER);
	st.graphics++;
}

void oracle_deint_leave_graphics(void)
{
	st.graphics--;
	record(EV_LEAVE);
}

float oracle_deint_sdr_white_level(void)
{
	record(EV_WHITE_LEVEL);
	return st.answers.white_level;
}

gs_eparam_t *oracle_deint_get_param_by_name(const gs_effect_t *effect, const char *name)
{
	struct oracle_deint_event *e = record(EV_PARAM);
	e->p[0] = effect;
	e->name = name;
	if (!effect || !st.answers.params_found)
		return NULL;
	for (size_t i = 0; i < ORACLE_DEINT_PARAMS; i++) {
		if (strcmp(name, param_names[i]) == 0)
			return oracle_deint_param(i);
	}
	return NULL;
}

gs_texture_t *oracle_deint_texrender_get_texture(const gs_texrender_t *texrender)
{
	struct oracle_deint_event *e = record(EV_TEXRENDER_TEXTURE);
	e->p[0] = texrender;
	if (!st.answers.texrender_texture)
		return NULL;
	return oracle_deint_texture(16 + (size_t)((const unsigned char *)texrender - texrenders));
}

bool oracle_deint_get_linear_srgb(void)
{
	record(EV_LINEAR_SRGB);
	return st.answers.linear_srgb;
}

enum gs_color_space oracle_deint_get_color_space(void)
{
	record(EV_COLOR_SPACE);
	return (enum gs_color_space)st.answers.color_space;
}

bool oracle_deint_framebuffer_srgb_enabled(void)
{
	record(EV_FRAMEBUFFER_SRGB);
	return st.answers.framebuffer_srgb;
}

void oracle_deint_enable_framebuffer_srgb(bool enable)
{
	struct oracle_deint_event *e = record(EV_ENABLE_FRAMEBUFFER_SRGB);
	e->i[0] = enable;
	st.answers.framebuffer_srgb = enable;
}

void oracle_deint_set_texture_srgb(gs_eparam_t *param, gs_texture_t *val)
{
	struct oracle_deint_event *e = record(EV_SET_TEXTURE_SRGB);
	e->p[0] = param;
	e->p[1] = val;
}

void oracle_deint_set_texture(gs_eparam_t *param, gs_texture_t *val)
{
	struct oracle_deint_event *e = record(EV_SET_TEXTURE);
	e->p[0] = param;
	e->p[1] = val;
}

void oracle_deint_set_float(gs_eparam_t *param, float val)
{
	struct oracle_deint_event *e = record(EV_SET_FLOAT);
	e->p[0] = param;
	e->f[0] = bits(val);
}

void oracle_deint_set_int(gs_eparam_t *param, int val)
{
	struct oracle_deint_event *e = record(EV_SET_INT);
	e->p[0] = param;
	e->i[0] = val;
}

void oracle_deint_set_vec2(gs_eparam_t *param, const struct vec2 *val)
{
	struct oracle_deint_event *e = record(EV_SET_VEC2);
	e->p[0] = param;
	e->f[0] = bits(val->x);
	e->f[1] = bits(val->y);
}

void oracle_deint_set_bool(gs_eparam_t *param, bool val)
{
	struct oracle_deint_event *e = record(EV_SET_BOOL);
	e->p[0] = param;
	e->i[0] = val;
}

bool oracle_deint_effect_loop(gs_effect_t *effect, const char *name)
{
	struct oracle_deint_event *e = record(EV_EFFECT_LOOP);
	e->p[0] = effect;
	e->name = name;
	if (st.answers.loops <= 0)
		return false;
	st.answers.loops--;
	return true;
}

void oracle_deint_draw_sprite(gs_texture_t *tex, uint32_t flip, uint32_t width, uint32_t height)
{
	struct oracle_deint_event *e = record(EV_DRAW_SPRITE);
	e->p[0] = tex;
	e->i[0] = (int)flip;
	e->i[1] = (int)width;
	e->i[2] = (int)height;
}

void oracle_deint_texture_destroy(gs_texture_t *tex)
{
	struct oracle_deint_event *e = record(EV_TEXTURE_DESTROY);
	e->p[0] = tex;
}

void oracle_deint_texrender_destroy(gs_texrender_t *texrender)
{
	struct oracle_deint_event *e = record(EV_TEXRENDER_DESTROY);
	e->p[0] = texrender;
}

/* The layouts the Rust shims mirror, measured from the real headers. */
size_t oracle_deint_vec2_size(void);
size_t oracle_deint_vec2_align(void);
size_t oracle_deint_vec2_offset_x(void);
size_t oracle_deint_vec2_offset_y(void);
size_t oracle_deint_fields_size(void);
size_t oracle_deint_fields_align(void);
size_t oracle_deint_fields_offset_mode(void);
size_t oracle_deint_fields_offset_top_first(void);
size_t oracle_deint_mode_size(void);
size_t oracle_deint_top_first_size(void);

size_t oracle_deint_vec2_size(void)
{
	return sizeof(struct vec2);
}

size_t oracle_deint_vec2_align(void)
{
	return _Alignof(struct vec2);
}

size_t oracle_deint_vec2_offset_x(void)
{
	return offsetof(struct vec2, x);
}

size_t oracle_deint_vec2_offset_y(void)
{
	return offsetof(struct vec2, y);
}

size_t oracle_deint_fields_size(void)
{
	return sizeof(struct obs_source_deinterlace_fields);
}

size_t oracle_deint_fields_align(void)
{
	return _Alignof(struct obs_source_deinterlace_fields);
}

size_t oracle_deint_fields_offset_mode(void)
{
	return offsetof(struct obs_source_deinterlace_fields, mode);
}

size_t oracle_deint_fields_offset_top_first(void)
{
	return offsetof(struct obs_source_deinterlace_fields, top_first);
}

size_t oracle_deint_mode_size(void)
{
	return sizeof(((struct obs_source *)NULL)->deinterlace_mode);
}

size_t oracle_deint_top_first_size(void)
{
	return sizeof(((struct obs_source *)NULL)->deinterlace_top_first);
}
