#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <graphics/matrix4.h>
#include <graphics/vec2.h>
#include <util/bmem.h>

/*
 * The immediate-mode vertex builder of libobs/graphics/graphics.c through
 * libobs's exported gs_render_start(), gs_vertex*, gs_normal3f(),
 * gs_color(), gs_texcoord(), gs_render_stop() and gs_render_save() (#3132).
 *
 * No GPU. The fixture builds a graphics_t by hand and enters it with
 * gs_enter_context, as test_texture_render does. Its exports are a fake
 * device: a vertex buffer owns the gs_vb_data it was made from and destroys
 * it with gs_vbdata_destroy(), and the immediate buffer holds
 * IMMEDIATE_COUNT vertices with texture array 0, as
 * graphics_init_immediate_vb() makes it. The test runs under ASan and
 * LeakSanitizer, which see every bmalloc() and bfree() of libobs.
 *
 * Covered: a draw in each mode; the stop and the save with no vertex leave
 * no vertex buffer data behind (a later save destroyed it a second time) and
 * free every array (the save dropped the normal, colour and texture arrays);
 * a save hands its data to the buffer and forgets it. These hold on the C,
 * the C reference and the hybrid libobs alike. The decisions the Rust port
 * changes (texture units, the buffer's count with a shorter array, the
 * texture arrays a save leaves) are in its parity test.
 */

#define IMMEDIATE 512

/* printf-checked, so clang's -Wformat-nonliteral accepts the vsnprintf. */
#if defined(__GNUC__) || defined(__clang__)
#define RECORD_FORMAT __attribute__((format(printf, 1, 2)))
#else
#define RECORD_FORMAT
#endif

struct fake_vb {
	struct gs_vb_data *data;
	bool immediate;
};

static char device_token;
static char call_log[4096];
static struct fake_vb immediate_vb;
static int buffers_alive;

RECORD_FORMAT static void record(const char *format, ...)
{
	size_t len = strlen(call_log);
	va_list args;

	if (len)
		call_log[len++] = ';';

	va_start(args, format);
	vsnprintf(call_log + len, sizeof(call_log) - len, format, args);
	va_end(args);
}

static void fake_enter_context(gs_device_t *device)
{
	(void)device;
}

static void fake_leave_context(gs_device_t *device)
{
	(void)device;
}

static gs_vertbuffer_t *fake_vertexbuffer_create(gs_device_t *device, struct gs_vb_data *data, uint32_t flags)
{
	struct fake_vb *vb = bzalloc(sizeof(*vb));

	(void)device;
	vb->data = data;
	buffers_alive++;
	record("create %zu tex%zu flags%u", data->num, data->num_tex, flags);
	return (gs_vertbuffer_t *)vb;
}

static void fake_vertexbuffer_destroy(gs_vertbuffer_t *vertbuffer)
{
	struct fake_vb *vb = (struct fake_vb *)vertbuffer;

	record("destroy");
	gs_vbdata_destroy(vb->data);
	bfree(vb);
	buffers_alive--;
}

static struct gs_vb_data *fake_vertexbuffer_get_data(const gs_vertbuffer_t *vertbuffer)
{
	return ((const struct fake_vb *)vertbuffer)->data;
}

static void fake_vertexbuffer_flush(gs_vertbuffer_t *vertbuffer)
{
	record("flush%s", ((struct fake_vb *)vertbuffer)->immediate ? " immediate" : "");
}

static void fake_load_vertexbuffer(gs_device_t *device, gs_vertbuffer_t *vertbuffer)
{
	(void)device;
	record("load%s", vertbuffer == (gs_vertbuffer_t *)&immediate_vb ? " immediate" : "");
}

static void fake_load_indexbuffer(gs_device_t *device, gs_indexbuffer_t *indexbuffer)
{
	(void)device;
	(void)indexbuffer;
}

static void fake_draw(gs_device_t *device, enum gs_draw_mode draw_mode, uint32_t start_vert, uint32_t num_verts)
{
	(void)device;
	record("draw %d %u %u", (int)draw_mode, start_vert, num_verts);
}

static graphics_t *graphics;

static int setup(void **state)
{
	struct matrix4 top;

	(void)state;

	graphics = bzalloc(sizeof(struct graphics_subsystem));
	matrix4_identity(&top);
	da_push_back(graphics->matrix_stack, &top);
	pthread_mutex_init(&graphics->mutex, NULL);
	pthread_mutex_init(&graphics->effect_mutex, NULL);

	graphics->device = (gs_device_t *)&device_token;
	graphics->exports.device_enter_context = fake_enter_context;
	graphics->exports.device_leave_context = fake_leave_context;
	graphics->exports.device_vertexbuffer_create = fake_vertexbuffer_create;
	graphics->exports.gs_vertexbuffer_destroy = fake_vertexbuffer_destroy;
	graphics->exports.gs_vertexbuffer_get_data = fake_vertexbuffer_get_data;
	graphics->exports.gs_vertexbuffer_flush = fake_vertexbuffer_flush;
	graphics->exports.device_load_vertexbuffer = fake_load_vertexbuffer;
	graphics->exports.device_load_indexbuffer = fake_load_indexbuffer;
	graphics->exports.device_draw = fake_draw;

	/* graphics_init_immediate_vb() */
	struct gs_vb_data *vbd = gs_vbdata_create();
	vbd->num = IMMEDIATE;
	vbd->points = bmalloc(sizeof(struct vec3) * IMMEDIATE);
	vbd->normals = bmalloc(sizeof(struct vec3) * IMMEDIATE);
	vbd->colors = bmalloc(sizeof(uint32_t) * IMMEDIATE);
	vbd->num_tex = 1;
	vbd->tvarray = bmalloc(sizeof(struct gs_tvertarray));
	vbd->tvarray[0].width = 2;
	vbd->tvarray[0].array = bmalloc(sizeof(struct vec2) * IMMEDIATE);
	immediate_vb.data = vbd;
	immediate_vb.immediate = true;
	graphics->immediate_vertbuffer = (gs_vertbuffer_t *)&immediate_vb;

	buffers_alive = 0;
	call_log[0] = 0;

	gs_enter_context(graphics);
	return 0;
}

static int teardown(void **state)
{
	(void)state;

	gs_leave_context();
	gs_vbdata_destroy(immediate_vb.data);
	immediate_vb.data = NULL;
	pthread_mutex_destroy(&graphics->mutex);
	pthread_mutex_destroy(&graphics->effect_mutex);
	da_free(graphics->matrix_stack);
	bfree(graphics);
	graphics = NULL;
	assert_int_equal(buffers_alive, 0);
	return 0;
}

static void quad(void)
{
	gs_vertex2f(0.0f, 0.0f);
	gs_vertex2f(1.0f, 0.0f);
	gs_vertex2f(0.0f, 1.0f);
	gs_vertex2f(1.0f, 1.0f);
}

/* ------------------------------------------------------------------------ */

static void a_new_buffer_draws_and_is_destroyed(void **state)
{
	(void)state;

	gs_render_start(true);
	quad();
	for (int i = 0; i < 4; i++) {
		gs_texcoord((float)i, 0.0f, 0);
		gs_color(0xff00ff00);
	}
	gs_render_stop(GS_TRISTRIP);

	assert_string_equal(call_log, "create 4 tex1 flags0;load;draw 4 0 0;destroy");
	assert_null(graphics->vbd);
}

static void immediate_mode_draws_from_its_buffer(void **state)
{
	(void)state;

	gs_render_start(false);
	quad();
	gs_texcoord(0.0f, 0.0f, 0);
	gs_texcoord(1.0f, 0.0f, 0);
	gs_texcoord(0.0f, 1.0f, 0);
	gs_texcoord(1.0f, 1.0f, 0);
	gs_render_stop(GS_TRISTRIP);

	assert_string_equal(call_log, "flush immediate;load immediate;draw 4 0 4");
	assert_null(graphics->vbd);
	assert_int_equal(graphics->verts.num, 0);
}

static void the_immediate_cap_drops_the_rest(void **state)
{
	(void)state;

	gs_render_start(false);
	for (int i = 0; i < IMMEDIATE + 3; i++)
		gs_vertex2f((float)i, 0.0f);
	assert_int_equal(graphics->verts.num, IMMEDIATE);
	gs_render_stop(GS_POINTS);

	assert_string_equal(call_log, "flush immediate;load immediate;draw 0 0 512");
}

static void an_empty_stop_leaves_no_data(void **state)
{
	(void)state;

	/* The stop destroys the new data; the save after it, with no start,
	 * destroyed it again. */
	gs_render_start(true);
	gs_render_stop(GS_TRIS);
	assert_null(graphics->vbd);
	assert_null(gs_render_save());
	assert_null(graphics->vbd);

	/* In immediate mode the buffer's data stays the buffer's. */
	gs_render_start(false);
	gs_render_stop(GS_TRIS);
	assert_null(graphics->vbd);

	assert_string_equal(call_log, "");
}

static void an_empty_save_frees_every_array(void **state)
{
	(void)state;

	/* Normals, colours and texture coordinates with no vertex: the save
	 * makes no buffer and frees them (LeakSanitizer checks at exit). */
	gs_render_start(true);
	gs_normal3f(0.0f, 0.0f, 1.0f);
	gs_color(0xffffffff);
	gs_texcoord(0.0f, 0.0f, 0);
	gs_texcoord(0.0f, 0.0f, 1);
	assert_null(gs_render_save());
	assert_null(graphics->vbd);
	assert_int_equal(graphics->norms.num, 0);
	assert_null(graphics->norms.array);
	assert_null(graphics->colors.array);
	assert_null(graphics->texverts[0].array);
	assert_null(graphics->texverts[1].array);

	assert_string_equal(call_log, "");
}

static void a_save_hands_its_data_over(void **state)
{
	(void)state;

	gs_render_start(true);
	quad();
	gs_vertbuffer_t *vb = gs_render_save();
	assert_non_null(vb);
	assert_null(graphics->vbd);

	/* A second save with no start makes nothing and leaves the buffer's
	 * data alone. */
	assert_null(gs_render_save());

	gs_vertexbuffer_destroy(vb);
	assert_string_equal(call_log, "create 4 tex0 flags0;destroy");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(a_new_buffer_draws_and_is_destroyed, setup, teardown),
		cmocka_unit_test_setup_teardown(immediate_mode_draws_from_its_buffer, setup, teardown),
		cmocka_unit_test_setup_teardown(the_immediate_cap_drops_the_rest, setup, teardown),
		cmocka_unit_test_setup_teardown(an_empty_stop_leaves_no_data, setup, teardown),
		cmocka_unit_test_setup_teardown(an_empty_save_frees_every_array, setup, teardown),
		cmocka_unit_test_setup_teardown(a_save_hands_its_data_over, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
