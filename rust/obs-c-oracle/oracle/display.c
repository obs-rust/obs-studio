/* Test-only oracle: the original libobs/obs-display.c.h, unmodified, with
 * its nine functions renamed to oracle_* so it can link next to the Rust
 * shim (rust/obs-core/src/ffi/display.rs).
 *
 * Its clear rules are the C oracle's (canvas_draw.c, under their oracle_*
 * names), so this side is C throughout. The display seam,
 * libobs/obs-display-seam.c.h, is compiled here under its own names, as it
 * stays C in libobs.core-source: the Rust shim reaches the fields through
 * it. The size fields (obs_display_size_fields(), _lock() and _unlock())
 * come from display_size.c.
 *
 * Both sides run on a world of their own: a zeroed struct obs_core with its
 * displays_mutex, whose graphics and solid effect are stand-in values.
 * `obs` (renamed oracle_display_obs) points at the world that runs; the Rust
 * parity test runs one world at a time, under its own lock. The calls into
 * the rest of libobs and the graphics subsystem (blog, the host's two clear
 * answers, obs_enter_graphics/_leave_graphics and the gs_* calls) are
 * renamed to the recording stand-ins below, which the parity test hands the
 * Rust shim too (its Calls): each one appends to the world's log. A swap
 * chain is a small number cast to a pointer, counted per world.
 *
 * The draw callback stand-in, oracle_display_draw(), logs its id, the size
 * it is given, and whether the rendering display's draw_callbacks_mutex is
 * held and its draw_info_mutex free, seen with trylock. */

#include <stddef.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define ORACLE_DISPLAY_PARAMS 4
#define ORACLE_DISPLAY_LOG 65536

/* The core libobs keeps in obs.c, under its own name. */
extern struct obs_core *oracle_display_obs;

void oracle_display_blog(int log_level, const char *format, ...);
bool oracle_display_host_clear_workaround(void);
bool oracle_display_host_clear_color_always_srgb(void);
void oracle_display_enter_graphics(void);
void oracle_display_leave_graphics(void);
void oracle_display_enter_context(graphics_t *graphics);
void oracle_display_leave_context(void);
gs_swapchain_t *oracle_display_swapchain_create(const struct gs_init_data *data);
void oracle_display_swapchain_destroy(gs_swapchain_t *swapchain);
void oracle_display_load_swapchain(gs_swapchain_t *swapchain);
void oracle_display_resize(uint32_t x, uint32_t y);
void oracle_display_update_color_space(void);
bool oracle_display_is_present_ready(void);
void oracle_display_begin_scene(void);
enum gs_color_space oracle_display_get_color_space(void);
void oracle_display_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil);
void oracle_display_enable_depth_test(bool enable);
void oracle_display_set_cull_mode(enum gs_cull_mode mode);
void oracle_display_ortho(float left, float right, float top, float bottom, float znear, float zfar);
void oracle_display_set_viewport(int x, int y, int width, int height);
gs_eparam_t *oracle_display_get_param_by_name(const gs_effect_t *effect, const char *name);
void oracle_display_effect_set_vec4(gs_eparam_t *param, const struct vec4 *val);
bool oracle_display_effect_loop(gs_effect_t *effect, const char *name);
void oracle_display_draw_sprite(gs_texture_t *tex, uint32_t flip, uint32_t width, uint32_t height);
void oracle_display_end_scene(void);
void oracle_display_present(void);

bool oracle_obs_display_clear_reads_space(bool always_srgb);
bool oracle_obs_display_clear_linear(int current_space);
uint32_t oracle_obs_display_clear_flags(bool use_clear_workaround);
bool oracle_obs_display_clear_draws_solid(bool use_clear_workaround);

bool oracle_obs_display_init(struct obs_display *display, const struct gs_init_data *graphics_data);
obs_display_t *oracle_obs_display_create(const struct gs_init_data *graphics_data, uint32_t background_color);
void oracle_obs_display_free(obs_display_t *display);
void oracle_obs_display_destroy(obs_display_t *display);
void oracle_obs_display_resize(obs_display_t *display, uint32_t cx, uint32_t cy);
void oracle_obs_display_update_color_space(obs_display_t *display);
void oracle_obs_display_add_draw_callback(obs_display_t *display, void (*draw)(void *param, uint32_t cx, uint32_t cy),
					  void *param);
void oracle_obs_display_remove_draw_callback(obs_display_t *display,
					     void (*draw)(void *param, uint32_t cx, uint32_t cy), void *param);
void oracle_render_display(struct obs_display *display);

#define obs oracle_display_obs
#define obs_display_init oracle_obs_display_init
#define obs_display_create oracle_obs_display_create
#define obs_display_free oracle_obs_display_free
#define obs_display_destroy oracle_obs_display_destroy
#define obs_display_resize oracle_obs_display_resize
#define obs_display_update_color_space oracle_obs_display_update_color_space
#define obs_display_add_draw_callback oracle_obs_display_add_draw_callback
#define obs_display_remove_draw_callback oracle_obs_display_remove_draw_callback
#define render_display oracle_render_display
#define obs_display_clear_reads_space oracle_obs_display_clear_reads_space
#define obs_display_clear_linear oracle_obs_display_clear_linear
#define obs_display_clear_flags oracle_obs_display_clear_flags
#define obs_display_clear_draws_solid oracle_obs_display_clear_draws_solid
#define blog oracle_display_blog
#define obs_host_display_clear_workaround oracle_display_host_clear_workaround
#define obs_host_display_clear_color_always_srgb oracle_display_host_clear_color_always_srgb
#define obs_enter_graphics oracle_display_enter_graphics
#define obs_leave_graphics oracle_display_leave_graphics
#define gs_enter_context oracle_display_enter_context
#define gs_leave_context oracle_display_leave_context
#define gs_swapchain_create oracle_display_swapchain_create
#define gs_swapchain_destroy oracle_display_swapchain_destroy
#define gs_load_swapchain oracle_display_load_swapchain
#define gs_resize oracle_display_resize
#define gs_update_color_space oracle_display_update_color_space
#define gs_is_present_ready oracle_display_is_present_ready
#define gs_begin_scene oracle_display_begin_scene
#define gs_get_color_space oracle_display_get_color_space
#define gs_clear oracle_display_clear
#define gs_enable_depth_test oracle_display_enable_depth_test
#define gs_set_cull_mode oracle_display_set_cull_mode
#define gs_ortho oracle_display_ortho
#define gs_set_viewport oracle_display_set_viewport
#define gs_effect_get_param_by_name oracle_display_get_param_by_name
#define gs_effect_set_vec4 oracle_display_effect_set_vec4
#define gs_effect_loop oracle_display_effect_loop
#define gs_draw_sprite oracle_display_draw_sprite
#define gs_end_scene oracle_display_end_scene
#define gs_present oracle_display_present

#include "obs-display.c.h"

#undef gs_present
#undef gs_end_scene
#undef gs_draw_sprite
#undef gs_effect_loop
#undef gs_effect_set_vec4
#undef gs_effect_get_param_by_name
#undef gs_set_viewport
#undef gs_ortho
#undef gs_set_cull_mode
#undef gs_enable_depth_test
#undef gs_clear
#undef gs_get_color_space
#undef gs_begin_scene
#undef gs_is_present_ready
#undef gs_update_color_space
#undef gs_resize
#undef gs_load_swapchain
#undef gs_swapchain_destroy
#undef gs_swapchain_create
#undef gs_leave_context
#undef gs_enter_context
#undef obs_leave_graphics
#undef obs_enter_graphics
#undef obs_host_display_clear_color_always_srgb
#undef obs_host_display_clear_workaround
#undef blog
#undef obs_display_clear_draws_solid
#undef obs_display_clear_flags
#undef obs_display_clear_linear
#undef obs_display_clear_reads_space
#undef render_display
#undef obs_display_remove_draw_callback
#undef obs_display_add_draw_callback
#undef obs_display_update_color_space
#undef obs_display_resize
#undef obs_display_destroy
#undef obs_display_free
#undef obs_display_create
#undef obs_display_init

/* `obs` stays renamed: the seam reads the world that runs. */
#include "obs-display-seam.c.h"

#undef obs

struct obs_core *oracle_display_obs;

/* The stand-in values of the core's graphics and solid effect, and what the
 * colour parameter lookup gives. */
#define FAKE_GRAPHICS ((graphics_t *)(uintptr_t)0x70)
#define FAKE_SOLID ((gs_effect_t *)(uintptr_t)0x90)
#define FAKE_PARAM ((gs_eparam_t *)(uintptr_t)0xb0)

struct oracle_display_param {
	struct oracle_display_world *world;
	int id;
};

/* What the stand-ins answer. */
struct oracle_display_config {
	bool workaround;
	bool always_srgb;
	bool present_ready;
	bool swap_fails;
	int space;
	uint32_t loops;
};

struct oracle_display_world {
	struct obs_core core;
	struct oracle_display_config config;
	uint32_t loops_left;
	uintptr_t next_swap;
	struct gs_init_data data;
	struct oracle_display_param params[ORACLE_DISPLAY_PARAMS];
	/* the display render_display() runs on, for the callback's locks */
	obs_display_t *rendering;
	char log[ORACLE_DISPLAY_LOG];
	size_t len;
};

void *oracle_display_new(void);
void oracle_display_free(void *world);
void oracle_display_use(void *world);
void oracle_display_configure(void *world, const struct oracle_display_config *config);
const struct gs_init_data *oracle_display_data(void *world, uint32_t cx, uint32_t cy);
void *oracle_display_param(void *world, int id);
void oracle_display_rendering(void *world, obs_display_t *display);
void oracle_display_draw(void *param, uint32_t cx, uint32_t cy);
void oracle_display_draw2(void *param, uint32_t cx, uint32_t cy);
void oracle_display_set_enabled(obs_display_t *display, bool enabled);
size_t oracle_display_take_log(void *world, char *out, size_t cap);
size_t oracle_display_state(void *world, obs_display_t *const *displays, size_t num, char *out, size_t cap);
size_t oracle_display_layout(size_t index);

static struct oracle_display_world *current;

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

static void log_str(const char *text)
{
	put_str(current->log, sizeof(current->log), &current->len, text);
}

static void log_hex(uint64_t value)
{
	log_str(" ");
	put_hex(current->log, sizeof(current->log), &current->len, value);
}

static void log_float(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	log_hex(bits);
}

static void log_end(void)
{
	log_str(";");
}

static void log_vec4(const struct vec4 *v)
{
	log_float(v->x);
	log_float(v->y);
	log_float(v->z);
	log_float(v->w);
}

/* Whether a mutex is free, seen from a stand-in. */
static bool mutex_free(pthread_mutex_t *mutex)
{
	if (pthread_mutex_trylock(mutex) != 0)
		return false;
	pthread_mutex_unlock(mutex);
	return true;
}

void oracle_display_blog(int log_level, const char *format, ...)
{
	log_str("blog");
	log_hex((uint64_t)(int64_t)log_level);
	log_str(" ");
	log_str(format);
	log_end();
}

bool oracle_display_host_clear_workaround(void)
{
	log_str("host_clear_workaround");
	log_end();
	return current->config.workaround;
}

bool oracle_display_host_clear_color_always_srgb(void)
{
	log_str("host_clear_color_always_srgb");
	log_end();
	return current->config.always_srgb;
}

void oracle_display_enter_graphics(void)
{
	log_str("enter_graphics");
	log_end();
}

void oracle_display_leave_graphics(void)
{
	log_str("leave_graphics");
	log_end();
}

void oracle_display_enter_context(graphics_t *graphics)
{
	log_str("enter_context");
	log_hex((uintptr_t)graphics);
	log_end();
}

void oracle_display_leave_context(void)
{
	log_str("leave_context");
	log_end();
}

gs_swapchain_t *oracle_display_swapchain_create(const struct gs_init_data *data)
{
	log_str("swapchain_create");
	log_hex(data == &current->data);
	log_hex(data->cx);
	log_hex(data->cy);
	log_end();
	if (current->config.swap_fails)
		return NULL;
	return (gs_swapchain_t *)++current->next_swap;
}

void oracle_display_swapchain_destroy(gs_swapchain_t *swapchain)
{
	log_str("swapchain_destroy");
	log_hex((uintptr_t)swapchain);
	log_end();
}

void oracle_display_load_swapchain(gs_swapchain_t *swapchain)
{
	log_str("load_swapchain");
	log_hex((uintptr_t)swapchain);
	log_end();
}

void oracle_display_resize(uint32_t x, uint32_t y)
{
	log_str("resize");
	log_hex(x);
	log_hex(y);
	log_end();
}

void oracle_display_update_color_space(void)
{
	log_str("update_color_space");
	log_end();
}

bool oracle_display_is_present_ready(void)
{
	log_str("is_present_ready");
	log_end();
	return current->config.present_ready;
}

void oracle_display_begin_scene(void)
{
	log_str("begin_scene");
	log_end();
}

enum gs_color_space oracle_display_get_color_space(void)
{
	log_str("get_color_space");
	log_end();
	return (enum gs_color_space)current->config.space;
}

void oracle_display_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil)
{
	log_str("clear");
	log_hex(clear_flags);
	log_vec4(color);
	log_float(depth);
	log_hex(stencil);
	log_end();
}

void oracle_display_enable_depth_test(bool enable)
{
	log_str("enable_depth_test");
	log_hex(enable);
	log_end();
}

void oracle_display_set_cull_mode(enum gs_cull_mode mode)
{
	log_str("set_cull_mode");
	log_hex((uint64_t)mode);
	log_end();
}

void oracle_display_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	log_str("ortho");
	log_float(left);
	log_float(right);
	log_float(top);
	log_float(bottom);
	log_float(znear);
	log_float(zfar);
	log_end();
}

void oracle_display_set_viewport(int x, int y, int width, int height)
{
	log_str("set_viewport");
	log_hex((uint64_t)(int64_t)x);
	log_hex((uint64_t)(int64_t)y);
	log_hex((uint64_t)(int64_t)width);
	log_hex((uint64_t)(int64_t)height);
	log_end();
}

gs_eparam_t *oracle_display_get_param_by_name(const gs_effect_t *effect, const char *name)
{
	log_str("get_param_by_name");
	log_hex((uintptr_t)effect);
	log_str(" ");
	log_str(name);
	log_end();
	return FAKE_PARAM;
}

void oracle_display_effect_set_vec4(gs_eparam_t *param, const struct vec4 *val)
{
	log_str("effect_set_vec4");
	log_hex((uintptr_t)param);
	log_vec4(val);
	log_end();
}

bool oracle_display_effect_loop(gs_effect_t *effect, const char *name)
{
	log_str("effect_loop");
	log_hex((uintptr_t)effect);
	log_str(" ");
	log_str(name);
	log_end();
	if (!current->loops_left) {
		current->loops_left = current->config.loops;
		return false;
	}
	current->loops_left--;
	return true;
}

void oracle_display_draw_sprite(gs_texture_t *tex, uint32_t flip, uint32_t width, uint32_t height)
{
	log_str("draw_sprite");
	log_hex((uintptr_t)tex);
	log_hex(flip);
	log_hex(width);
	log_hex(height);
	log_end();
}

void oracle_display_end_scene(void)
{
	log_str("end_scene");
	log_end();
}

void oracle_display_present(void)
{
	log_str("present");
	log_end();
}

static void log_draw(const char *what, void *param, uint32_t cx, uint32_t cy)
{
	struct oracle_display_param *p = param;
	obs_display_t *display = p->world->rendering;

	log_str(what);
	log_hex((uint64_t)(int64_t)p->id);
	log_hex(cx);
	log_hex(cy);
	if (display) {
		log_hex(!mutex_free(&display->draw_callbacks_mutex));
		log_hex(mutex_free(&display->draw_info_mutex));
	}
	log_end();
}

void oracle_display_draw(void *param, uint32_t cx, uint32_t cy)
{
	log_draw("draw", param, cx, cy);
}

/* A second function, so a remove matches the pair, not the param alone. */
void oracle_display_draw2(void *param, uint32_t cx, uint32_t cy)
{
	log_draw("draw2", param, cx, cy);
}

void *oracle_display_new(void)
{
	struct oracle_display_world *world = bzalloc(sizeof(*world));

	pthread_mutex_init(&world->core.data.displays_mutex, NULL);
	world->core.video.graphics = FAKE_GRAPHICS;
	world->core.video.solid_effect = FAKE_SOLID;
	for (int i = 0; i < ORACLE_DISPLAY_PARAMS; i++) {
		world->params[i].world = world;
		world->params[i].id = i;
	}
	world->config.present_ready = true;
	return world;
}

void oracle_display_free(void *opaque)
{
	struct oracle_display_world *world = opaque;

	if (current == world) {
		current = NULL;
		oracle_display_obs = NULL;
	}
	pthread_mutex_destroy(&world->core.data.displays_mutex);
	bfree(world);
}

void oracle_display_use(void *opaque)
{
	current = opaque;
	oracle_display_obs = &current->core;
}

void oracle_display_configure(void *opaque, const struct oracle_display_config *config)
{
	struct oracle_display_world *world = opaque;

	world->config = *config;
	world->loops_left = config->loops;
}

const struct gs_init_data *oracle_display_data(void *opaque, uint32_t cx, uint32_t cy)
{
	struct oracle_display_world *world = opaque;

	memset(&world->data, 0, sizeof(world->data));
	world->data.cx = cx;
	world->data.cy = cy;
	return &world->data;
}

void *oracle_display_param(void *opaque, int id)
{
	struct oracle_display_world *world = opaque;

	return &world->params[id];
}

void oracle_display_rendering(void *opaque, obs_display_t *display)
{
	struct oracle_display_world *world = opaque;

	world->rendering = display;
}

void oracle_display_set_enabled(obs_display_t *display, bool enabled)
{
	display->enabled = enabled;
}

size_t oracle_display_take_log(void *opaque, char *out, size_t cap)
{
	struct oracle_display_world *world = opaque;
	size_t len = world->len < cap ? world->len : cap;

	memcpy(out, world->log, len);
	world->len = 0;
	world->log[0] = 0;
	return len;
}

static int64_t index_of(obs_display_t *const *displays, size_t num, const obs_display_t *display)
{
	if (!display)
		return -1;
	for (size_t i = 0; i < num; i++) {
		if (displays[i] == display)
			return (int64_t)i;
	}
	return -2;
}

/* The list from the head (each display as its index in `displays`, and
 * where its prev_next points: the head, or the next of the display before;
 * a walk longer than the displays made is a cycle, and stops),
 * then every live display's fields. */
size_t oracle_display_state(void *opaque, obs_display_t *const *displays, size_t num, char *out, size_t cap)
{
	struct oracle_display_world *world = opaque;
	size_t len = 0;

	out[0] = 0;
	put_str(out, cap, &len, "list");
	obs_display_t **link = &world->core.data.first_display;
	size_t steps = 0;
	for (obs_display_t *d = world->core.data.first_display; d; d = d->next) {
		if (++steps > num + 1) {
			put_str(out, cap, &len, " cycle");
			break;
		}
		put_str(out, cap, &len, " ");
		put_hex(out, cap, &len, (uint64_t)index_of(displays, num, d));
		put_str(out, cap, &len, d->prev_next == link ? "+" : "!");
		link = &d->next;
	}
	put_str(out, cap, &len, ";");
	for (size_t i = 0; i < num; i++) {
		obs_display_t *d = displays[i];
		if (!d)
			continue;
		const uint32_t values[] = {d->cx,
					   d->cy,
					   d->next_cx,
					   d->next_cy,
					   d->update_color_space,
					   d->enabled,
					   d->background_color,
					   d->use_clear_workaround,
					   (uint32_t)(uintptr_t)d->swap,
					   (uint32_t)d->draw_callbacks.num};
		put_str(out, cap, &len, "display ");
		put_hex(out, cap, &len, i);
		for (size_t v = 0; v < sizeof(values) / sizeof(values[0]); v++) {
			put_str(out, cap, &len, " ");
			put_hex(out, cap, &len, values[v]);
		}
		for (size_t c = 0; c < d->draw_callbacks.num; c++) {
			const struct draw_callback *cb = d->draw_callbacks.array + c;
			const struct oracle_display_param *p = cb->param;
			put_str(out, cap, &len,
				cb->draw == oracle_display_draw    ? " cb"
				: cb->draw == oracle_display_draw2 ? " cb2"
								   : " other");
			put_hex(out, cap, &len, p ? (uint64_t)(int64_t)p->id : UINT64_MAX);
		}
		put_str(out, cap, &len, mutex_free(&d->draw_callbacks_mutex) ? " free" : " held");
		put_str(out, cap, &len, mutex_free(&d->draw_info_mutex) ? " free" : " held");
		put_str(out, cap, &len, ";");
	}
	return len;
}

size_t oracle_display_layout(size_t index)
{
	const size_t layout[] = {
		sizeof(struct draw_callback),
		_Alignof(struct draw_callback),
		offsetof(struct draw_callback, draw),
		offsetof(struct draw_callback, param),
		sizeof(struct vec4),
		_Alignof(struct vec4),
		offsetof(struct vec4, x),
		offsetof(struct vec4, y),
		offsetof(struct vec4, z),
		offsetof(struct vec4, w),
		sizeof(struct obs_display),
		sizeof(struct oracle_display_config),
		offsetof(struct oracle_display_config, space),
		offsetof(struct oracle_display_config, loops),
	};
	return index < sizeof(layout) / sizeof(layout[0]) ? layout[index] : SIZE_MAX;
}
