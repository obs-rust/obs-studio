/* Test-only oracle: the original libobs/obs-source-transition-video.c.h,
 * unmodified, with its four functions renamed to oracle_* so they can link
 * next to the Rust shims
 * (rust/obs-source-seam/src/ffi/source_transition_video.rs).
 *
 * The render rules (obs_source_transition_render_*) and the colour-space
 * mix (obs_source_transition_clock_mix_spaces) are the Rust ones, as in the
 * hybrid libobs, and transition_valid() is the transition seam's, compiled
 * under libobs's names by oracle/source_transition_settings.c; the C reads
 * the fields and takes both mutexes directly, the Rust shims through the
 * seam. The C's other calls go to recording stand-ins below, which the
 * parity test hands the Rust shims too (their Calls): the clock read and
 * size helpers (a set time and size), the stop (the stop oracle's
 * oracle_transition_stop(), on the slots oracle's graph,
 * oracle/source_transition_slots.c), the stop handler, obs_source_active(),
 * obs_source_get_ref() and obs_source_release() (the slots oracle's), the
 * signal, the colour-space reads, the child render, and every graphics call
 * (fake texrenders and textures, each a byte of a static array, whose
 * format, texture and begin result a test sets). The core's transparent
 * texture is this file's own core pointer, obs renamed from the include on.
 *
 * Each stand-in appends one event to a log, in order: what it was called
 * with (sources, texrenders and textures as pointers, the enums as int, the
 * floats bit for bit) and whether the watched transition's
 * transition_mutex (bit 0) and transition_tex_mutex (bit 1) were held. The
 * draw callback, oracle_video_callback(), logs the same way. */

#include "libobs/build/libobs-ported-source-transition-video.pch.h"

void oracle_obs_transition_video_render(obs_source_t *transition, obs_transition_video_render_callback_t callback);
void oracle_obs_transition_video_render2(obs_source_t *transition, obs_transition_video_render_callback_t callback,
					 gs_texture_t *placeholder_texture);
enum gs_color_space oracle_obs_transition_video_get_color_space(obs_source_t *transition);
bool oracle_obs_transition_video_render_direct(obs_source_t *transition, enum obs_transition_target target);

float oracle_video_time(obs_source_t *transition);
void oracle_video_stop(obs_source_t *transition);
void oracle_video_handle_stop(obs_source_t *transition);
uint32_t oracle_video_cx(obs_source_t *transition);
uint32_t oracle_video_cy(obs_source_t *transition);
bool oracle_video_active(const obs_source_t *source);
obs_source_t *oracle_video_get_ref(obs_source_t *source);
void oracle_video_release(obs_source_t *source);
void oracle_video_dosignal(obs_source_t *source, const char *signal_obs, const char *signal_source);
enum gs_color_space oracle_video_color_space(obs_source_t *source, size_t count,
					     const enum gs_color_space *preferred_spaces);
void oracle_video_render(obs_source_t *source);
gs_texture_t *oracle_video_texrender_get_texture(const gs_texrender_t *texrender);
enum gs_color_format oracle_video_texrender_get_format(const gs_texrender_t *texrender);
void oracle_video_texrender_destroy(gs_texrender_t *texrender);
gs_texrender_t *oracle_video_texrender_create(enum gs_color_format format, enum gs_zstencil_format zsformat);
bool oracle_video_texrender_begin(gs_texrender_t *texrender, uint32_t cx, uint32_t cy, enum gs_color_space space);
void oracle_video_texrender_end(gs_texrender_t *texrender);
enum gs_color_format oracle_video_format_from_space(enum gs_color_space space);
enum gs_color_space oracle_video_current_color_space(void);
void oracle_video_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil);
void oracle_video_ortho(float left, float right, float top, float bottom, float znear, float zfar);
void oracle_video_matrix_push(void);
void oracle_video_matrix_mul(const struct matrix4 *matrix);
void oracle_video_matrix_pop(void);
void oracle_video_blend_state_push(void);
void oracle_video_blend_function(enum gs_blend_type src, enum gs_blend_type dest);
void oracle_video_blend_state_pop(void);

extern struct obs_core *oracle_video_obs;

#define obs_transition_video_render oracle_obs_transition_video_render
#define obs_transition_video_render2 oracle_obs_transition_video_render2
#define obs_transition_video_get_color_space oracle_obs_transition_video_get_color_space
#define obs_transition_video_render_direct oracle_obs_transition_video_render_direct
#define transition_get_video_time oracle_video_time
#define transition_stop oracle_video_stop
#define transition_handle_stop oracle_video_handle_stop
#define transition_get_cx oracle_video_cx
#define transition_get_cy oracle_video_cy
#define obs_source_active oracle_video_active
#define obs_source_get_ref oracle_video_get_ref
#define obs_source_release oracle_video_release
#define obs_source_dosignal oracle_video_dosignal
#define obs_source_get_color_space oracle_video_color_space
#define obs_source_video_render oracle_video_render
#define gs_texrender_get_texture oracle_video_texrender_get_texture
#define gs_texrender_get_format oracle_video_texrender_get_format
#define gs_texrender_destroy oracle_video_texrender_destroy
#define gs_texrender_create oracle_video_texrender_create
#define gs_texrender_begin_with_color_space oracle_video_texrender_begin
#define gs_texrender_end oracle_video_texrender_end
#define gs_get_format_from_space oracle_video_format_from_space
#define gs_get_color_space oracle_video_current_color_space
#define gs_clear oracle_video_clear
#define gs_ortho oracle_video_ortho
#define gs_matrix_push oracle_video_matrix_push
#define gs_matrix_mul oracle_video_matrix_mul
#define gs_matrix_pop oracle_video_matrix_pop
#define gs_blend_state_push oracle_video_blend_state_push
#define gs_blend_function oracle_video_blend_function
#define gs_blend_state_pop oracle_video_blend_state_pop
#define obs oracle_video_obs

#include "obs-source-transition-video.c.h"

#undef obs
#undef gs_blend_state_pop
#undef gs_blend_function
#undef gs_blend_state_push
#undef gs_matrix_pop
#undef gs_matrix_mul
#undef gs_matrix_push
#undef gs_ortho
#undef gs_clear
#undef gs_get_color_space
#undef gs_get_format_from_space
#undef gs_texrender_end
#undef gs_texrender_begin_with_color_space
#undef gs_texrender_create
#undef gs_texrender_destroy
#undef gs_texrender_get_format
#undef gs_texrender_get_texture
#undef obs_source_video_render
#undef obs_source_get_color_space
#undef obs_source_dosignal
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_source_active
#undef transition_get_cy
#undef transition_get_cx
#undef transition_handle_stop
#undef transition_stop
#undef transition_get_video_time
#undef obs_transition_video_render_direct
#undef obs_transition_video_get_color_space
#undef obs_transition_video_render2
#undef obs_transition_video_render

/* The stop oracle's transition_stop(), renamed there
 * (oracle/source_transition_stop.c). */
void oracle_transition_stop(obs_source_t *transition);
obs_source_t *oracle_slots_get_ref(obs_source_t *source);
void oracle_slots_release(obs_source_t *source);

/* The event kinds. */
enum {
	EV_VIDEO_TIME = 1,
	EV_STOP,
	EV_HANDLE_STOP,
	EV_CX,
	EV_CY,
	EV_ACTIVE,
	EV_GET_REF,
	EV_RELEASE,
	EV_SIGNAL,
	EV_COLOR_SPACE,
	EV_RENDER,
	EV_TEX_TEXTURE,
	EV_TEX_FORMAT,
	EV_TEX_DESTROY,
	EV_TEX_CREATE,
	EV_TEX_BEGIN,
	EV_TEX_END,
	EV_FORMAT_FROM_SPACE,
	EV_CURRENT_SPACE,
	EV_CLEAR,
	EV_ORTHO,
	EV_PUSH,
	EV_MUL,
	EV_POP,
	EV_BLEND_PUSH,
	EV_BLEND_FUNCTION,
	EV_BLEND_POP,
	EV_CALLBACK,
};

#define ORACLE_VIDEO_EVENTS 256
#define ORACLE_VIDEO_HANDLES 32
#define ORACLE_VIDEO_SPACES 8

/* One call: its kind, up to three pointers, up to four ints, up to sixteen
 * floats (as bits), and the watched transition's held mutexes. */
struct oracle_video_event {
	int kind;
	const void *p[3];
	const char *name[2];
	int i[4];
	uint32_t f[16];
	int held;
};

static struct obs_core oracle_video_core;
struct obs_core *oracle_video_obs = &oracle_video_core;

static unsigned char texrenders[ORACLE_VIDEO_HANDLES];
static unsigned char textures[ORACLE_VIDEO_HANDLES];

static struct {
	obs_source_t *watched;
	float time;
	uint32_t cx;
	uint32_t cy;
	bool active;
	int current_space;
	bool begins;
	/* the space each source gives obs_source_get_color_space() */
	obs_source_t *space_source[ORACLE_VIDEO_SPACES];
	int space[ORACLE_VIDEO_SPACES];
	size_t spaces;
	int format[ORACLE_VIDEO_HANDLES];
	bool no_texture[ORACLE_VIDEO_HANDLES];
	size_t next_texrender;
	struct oracle_video_event log[ORACLE_VIDEO_EVENTS];
	size_t logged;
} st;

void oracle_video_reset(obs_source_t *watched, float time, uint32_t cx, uint32_t cy, bool active, int current_space,
			bool begins, size_t next_texrender);
void oracle_video_set_space(obs_source_t *source, int space);
void oracle_video_set_texrender(size_t n, int format, bool no_texture);
gs_texrender_t *oracle_video_texrender(size_t n);
gs_texture_t *oracle_video_texture(size_t n);
void oracle_video_set_transparent(gs_texture_t *texture);
gs_texture_t *oracle_video_transparent_texture(void);
void oracle_video_callback(void *data, gs_texture_t *a, gs_texture_t *b, float t, uint32_t cx, uint32_t cy);
size_t oracle_video_logged(void);
const struct oracle_video_event *oracle_video_event(size_t i);
enum gs_color_format oracle_gs_get_format_from_space(int space);

/* Watches transition, and sets what the stand-ins give: the time, size,
 * active state, current colour space, the texrender begin result and the
 * first texrender a create hands out. Forgets the spaces, the texrender
 * formats and textures, and the log. */
void oracle_video_reset(obs_source_t *watched, float time, uint32_t cx, uint32_t cy, bool active, int current_space,
			bool begins, size_t next_texrender)
{
	memset(&st, 0, sizeof(st));
	st.watched = watched;
	st.time = time;
	st.cx = cx;
	st.cy = cy;
	st.active = active;
	st.current_space = current_space;
	st.begins = begins;
	st.next_texrender = next_texrender;
	oracle_video_core.video.transparent_texture = NULL;
}

void oracle_video_set_space(obs_source_t *source, int space)
{
	if (st.spaces < ORACLE_VIDEO_SPACES) {
		st.space_source[st.spaces] = source;
		st.space[st.spaces] = space;
		st.spaces++;
	}
}

void oracle_video_set_texrender(size_t n, int format, bool no_texture)
{
	if (n < ORACLE_VIDEO_HANDLES) {
		st.format[n] = format;
		st.no_texture[n] = no_texture;
	}
}

/* Fake texrender (and texture) n: the address of byte n of a static array.
 * Never NULL. */
gs_texrender_t *oracle_video_texrender(size_t n)
{
	return (gs_texrender_t *)&texrenders[n % ORACLE_VIDEO_HANDLES];
}

gs_texture_t *oracle_video_texture(size_t n)
{
	return (gs_texture_t *)&textures[n % ORACLE_VIDEO_HANDLES];
}

void oracle_video_set_transparent(gs_texture_t *texture)
{
	oracle_video_core.video.transparent_texture = texture;
}

gs_texture_t *oracle_video_transparent_texture(void)
{
	return oracle_video_core.video.transparent_texture;
}

size_t oracle_video_logged(void)
{
	return st.logged;
}

const struct oracle_video_event *oracle_video_event(size_t i)
{
	return i < st.logged ? &st.log[i] : NULL;
}

/* The header inline, instantiated for the parity test. */
enum gs_color_format oracle_gs_get_format_from_space(int space)
{
	return gs_get_format_from_space((enum gs_color_space)space);
}

static int held(void)
{
	int h = 0;

	if (!st.watched)
		return -1;
	if (pthread_mutex_trylock(&st.watched->transition_mutex) != 0)
		h |= 1;
	else
		pthread_mutex_unlock(&st.watched->transition_mutex);
	if (pthread_mutex_trylock(&st.watched->transition_tex_mutex) != 0)
		h |= 2;
	else
		pthread_mutex_unlock(&st.watched->transition_tex_mutex);
	return h;
}

static struct oracle_video_event *record(int kind)
{
	static struct oracle_video_event overflow;
	struct oracle_video_event *e = st.logged < ORACLE_VIDEO_EVENTS ? &st.log[st.logged++] : &overflow;

	memset(e, 0, sizeof(*e));
	e->kind = kind;
	e->held = held();
	return e;
}

static uint32_t bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static size_t index_of(const void *p, const unsigned char *array)
{
	const unsigned char *b = p;
	return (b >= array && b < array + ORACLE_VIDEO_HANDLES) ? (size_t)(b - array) : ORACLE_VIDEO_HANDLES;
}

/* The stand-ins */

float oracle_video_time(obs_source_t *transition)
{
	record(EV_VIDEO_TIME)->p[0] = transition;
	return st.time;
}

void oracle_video_stop(obs_source_t *transition)
{
	record(EV_STOP)->p[0] = transition;
	oracle_transition_stop(transition);
}

void oracle_video_handle_stop(obs_source_t *transition)
{
	record(EV_HANDLE_STOP)->p[0] = transition;
}

uint32_t oracle_video_cx(obs_source_t *transition)
{
	record(EV_CX)->p[0] = transition;
	return st.cx;
}

uint32_t oracle_video_cy(obs_source_t *transition)
{
	record(EV_CY)->p[0] = transition;
	return st.cy;
}

bool oracle_video_active(const obs_source_t *source)
{
	record(EV_ACTIVE)->p[0] = source;
	return st.active;
}

obs_source_t *oracle_video_get_ref(obs_source_t *source)
{
	obs_source_t *got;
	struct oracle_video_event *e = record(EV_GET_REF);

	e->p[0] = source;
	got = source ? oracle_slots_get_ref(source) : NULL;
	e->p[1] = got;
	return got;
}

void oracle_video_release(obs_source_t *source)
{
	record(EV_RELEASE)->p[0] = source;
	if (source)
		oracle_slots_release(source);
}

void oracle_video_dosignal(obs_source_t *source, const char *signal_obs, const char *signal_source)
{
	struct oracle_video_event *e = record(EV_SIGNAL);

	e->p[0] = source;
	e->name[0] = signal_obs;
	e->name[1] = signal_source;
}

enum gs_color_space oracle_video_color_space(obs_source_t *source, size_t count,
					     const enum gs_color_space *preferred_spaces)
{
	struct oracle_video_event *e = record(EV_COLOR_SPACE);

	e->p[0] = source;
	e->i[0] = (int)count;
	for (size_t i = 0; i < count && i < 3; i++)
		e->i[1 + i] = (int)preferred_spaces[i];
	for (size_t i = 0; i < st.spaces; i++) {
		if (st.space_source[i] == source)
			return (enum gs_color_space)st.space[i];
	}
	return GS_CS_SRGB;
}

void oracle_video_render(obs_source_t *source)
{
	record(EV_RENDER)->p[0] = source;
}

gs_texture_t *oracle_video_texrender_get_texture(const gs_texrender_t *texrender)
{
	size_t n = index_of(texrender, texrenders);

	record(EV_TEX_TEXTURE)->p[0] = texrender;
	if (n == ORACLE_VIDEO_HANDLES || st.no_texture[n])
		return NULL;
	return oracle_video_texture(n);
}

enum gs_color_format oracle_video_texrender_get_format(const gs_texrender_t *texrender)
{
	size_t n = index_of(texrender, texrenders);

	record(EV_TEX_FORMAT)->p[0] = texrender;
	return n == ORACLE_VIDEO_HANDLES ? GS_UNKNOWN : (enum gs_color_format)st.format[n];
}

void oracle_video_texrender_destroy(gs_texrender_t *texrender)
{
	record(EV_TEX_DESTROY)->p[0] = texrender;
}

gs_texrender_t *oracle_video_texrender_create(enum gs_color_format format, enum gs_zstencil_format zsformat)
{
	struct oracle_video_event *e = record(EV_TEX_CREATE);
	size_t n = st.next_texrender++ % ORACLE_VIDEO_HANDLES;

	e->i[0] = (int)format;
	e->i[1] = (int)zsformat;
	st.format[n] = (int)format;
	st.no_texture[n] = false;
	e->p[0] = oracle_video_texrender(n);
	return oracle_video_texrender(n);
}

bool oracle_video_texrender_begin(gs_texrender_t *texrender, uint32_t cx, uint32_t cy, enum gs_color_space space)
{
	struct oracle_video_event *e = record(EV_TEX_BEGIN);

	e->p[0] = texrender;
	e->i[0] = (int)cx;
	e->i[1] = (int)cy;
	e->i[2] = (int)space;
	return st.begins;
}

void oracle_video_texrender_end(gs_texrender_t *texrender)
{
	record(EV_TEX_END)->p[0] = texrender;
}

enum gs_color_format oracle_video_format_from_space(enum gs_color_space space)
{
	record(EV_FORMAT_FROM_SPACE)->i[0] = (int)space;
	return gs_get_format_from_space(space);
}

enum gs_color_space oracle_video_current_color_space(void)
{
	record(EV_CURRENT_SPACE);
	return (enum gs_color_space)st.current_space;
}

void oracle_video_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil)
{
	struct oracle_video_event *e = record(EV_CLEAR);

	e->i[0] = (int)clear_flags;
	e->i[1] = stencil;
	e->f[0] = bits(color->x);
	e->f[1] = bits(color->y);
	e->f[2] = bits(color->z);
	e->f[3] = bits(color->w);
	e->f[4] = bits(depth);
}

void oracle_video_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	struct oracle_video_event *e = record(EV_ORTHO);

	e->f[0] = bits(left);
	e->f[1] = bits(right);
	e->f[2] = bits(top);
	e->f[3] = bits(bottom);
	e->f[4] = bits(znear);
	e->f[5] = bits(zfar);
}

void oracle_video_matrix_push(void)
{
	record(EV_PUSH);
}

void oracle_video_matrix_mul(const struct matrix4 *matrix)
{
	struct oracle_video_event *e = record(EV_MUL);
	const float *m = (const float *)matrix;

	for (size_t i = 0; i < 16; i++)
		e->f[i] = bits(m[i]);
}

void oracle_video_matrix_pop(void)
{
	record(EV_POP);
}

void oracle_video_blend_state_push(void)
{
	record(EV_BLEND_PUSH);
}

void oracle_video_blend_function(enum gs_blend_type src, enum gs_blend_type dest)
{
	struct oracle_video_event *e = record(EV_BLEND_FUNCTION);

	e->i[0] = (int)src;
	e->i[1] = (int)dest;
}

void oracle_video_blend_state_pop(void)
{
	record(EV_BLEND_POP);
}

void oracle_video_callback(void *data, gs_texture_t *a, gs_texture_t *b, float t, uint32_t cx, uint32_t cy)
{
	struct oracle_video_event *e = record(EV_CALLBACK);

	e->p[0] = data;
	e->p[1] = a;
	e->p[2] = b;
	e->i[0] = (int)cx;
	e->i[1] = (int)cy;
	e->f[0] = bits(t);
}
