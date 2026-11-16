/* Public immediate argument adapters, exercised without a graphics module or GPU.
 * A live context without gs_render_start is characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "graphics/graphics-immediate.h"
#include "graphics/vec2.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static unsigned logs;
static int last_level;
static char last_log[160];
static log_handler_t saved_handler;
static void *saved_param;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	logs++;
	last_level = level;
	vsnprintf(last_log, sizeof(last_log), format, args);
}

static void context(gs_device_t *device)
{
	assert_null(device);
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	logs = 0;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	da_free(graphics.verts);
	da_free(graphics.norms);
	da_free(graphics.colors);
	for (unsigned i = 0; i < 16; i++)
		da_free(graphics.texverts[i]);
	base_set_log_handler(saved_handler, saved_param);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static float from_bits(uint32_t bits)
{
	float value;
	memcpy(&value, &bits, sizeof(value));
	return value;
}

static void same_bits(float actual, uint32_t expected)
{
	uint32_t bits;
	memcpy(&bits, &actual, sizeof(bits));
	assert_int_equal(bits, expected);
}

static void diagnostic(const char *name)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(graphics.verts.num, 0);
	for (unsigned i = 0; i < 16; i++)
		assert_int_equal(graphics.texverts[i].num, 0);
	logs = 0;
}

static void missing_context(void **state)
{
	(void)state;
	const struct vec2 input = {{.x = -0.0f, .y = 2.5f}};
	const int units[] = {INT_MIN, -1, 0, 1, 15, 16, INT_MAX};
	for (unsigned phase = 0; phase < 2; phase++) {
		if (phase) {
			gs_enter_context(&graphics);
			gs_leave_context();
		}
		gs_vertex3f(-0.0f, 2.5f, -3.0f);
		diagnostic("gs_vertex3v");
		/* The input is read before downstream context validation; NULL is outside the domain. */
		gs_vertex2v(&input);
		diagnostic("gs_vertex3v");
		for (unsigned i = 0; i < sizeof(units) / sizeof(units[0]); i++) {
			gs_texcoord(input.x, input.y, units[i]);
			diagnostic("gs_texcoord2v");
		}
	}
}

/* Quiet NaN payloads, infinities, signed zeros, subnormals and finite extremes.
 * memcpy avoids aliasing and truncating constant casts on MSVC. */
static const uint32_t components[][3] = {
	{0x00000000u, 0x80000000u, 0x3f800000u}, {0x80000000u, 0x00000000u, 0xbf800000u},
	{0x7f800000u, 0xff800000u, 0x7fc12345u}, {0xffc54321u, 0x7fc12345u, 0x80000000u},
	{0x00000001u, 0x80000001u, 0x7f7fffffu}, {0xff7fffffu, 0x3fa00000u, 0xc0200000u},
};

static void vertex_bits(const struct vec3 *v, uint32_t x, uint32_t y, uint32_t z)
{
	same_bits(v->x, x);
	same_bits(v->y, y);
	same_bits(v->z, z);
	same_bits(v->w, 0);
}

static void argument_forwarding(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	assert_false(graphics.using_immediate);
	assert_null(graphics.vbd);
	/* No render_start: these exports append even without an active render batch. */
	for (unsigned i = 0; i < sizeof(components) / sizeof(components[0]); i++) {
		float x = from_bits(components[i][0]);
		float y = from_bits(components[i][1]);
		float z = from_bits(components[i][2]);
		struct vec2 input;
		vec2_set(&input, x, y);
		struct vec2 original = input;
		size_t before = graphics.verts.num;
		gs_vertex3f(x, y, z);
		assert_int_equal(graphics.verts.num, before + 1);
		vertex_bits(&graphics.verts.array[before], components[i][0], components[i][1], components[i][2]);
		gs_vertex2v(&input);
		assert_int_equal(graphics.verts.num, before + 2);
		vertex_bits(&graphics.verts.array[before + 1], components[i][0], components[i][1], 0);
		assert_memory_equal(&input, &original, sizeof(input));
		/* Descending units detects accidental default-unit routing or normalization. */
		for (int unit = 15; unit >= 0; unit--) {
			size_t counts[16];
			for (unsigned j = 0; j < 16; j++)
				counts[j] = graphics.texverts[j].num;
			gs_texcoord(x, y, unit);
			for (int j = 0; j < 16; j++)
				assert_int_equal(graphics.texverts[j].num, counts[j] + (j == unit ? 1 : 0));
			const struct vec2 *stored = &graphics.texverts[unit].array[counts[unit]];
			same_bits(stored->x, components[i][0]);
			same_bits(stored->y, components[i][1]);
		}
		assert_int_equal(graphics.verts.num, before + 2);
		/* Later calls must not overwrite previously appended values. */
		vertex_bits(&graphics.verts.array[before], components[i][0], components[i][1], components[i][2]);
		assert_int_equal(graphics.norms.num, 0);
		assert_int_equal(graphics.colors.num, 0);
		assert_int_equal(logs, 0);
	}
}

/* The probe compiles these exact C bodies with only the public callees renamed.
 * It observes every call without indexing the live context's bounded arrays. */
void immediate_probe_vertex3f(float x, float y, float z);
void immediate_probe_texcoord(float x, float y, int unit);
void immediate_probe_vertex2v(const struct vec2 *v);
void immediate_record_vertex(const struct vec3 *v);
void immediate_record_texcoord(const struct vec2 *v, int unit);

static struct vec3 recorded_vertex;
static struct vec2 recorded_texcoord;
static int recorded_unit;
static unsigned vertex_calls, texcoord_calls;

void immediate_record_vertex(const struct vec3 *v)
{
	vertex_calls++;
	memcpy(&recorded_vertex, v, sizeof(*v));
}

void immediate_record_texcoord(const struct vec2 *v, int unit)
{
	texcoord_calls++;
	recorded_unit = unit;
	memcpy(&recorded_texcoord, v, sizeof(*v));
}

static void exact_downstream_calls(void **state)
{
	(void)state;
	const int units[] = {INT_MIN, -65537, -1, 0, 1, 7, 15, 16, 65537, INT_MAX};
	vertex_calls = texcoord_calls = 0;
	for (unsigned i = 0; i < sizeof(components) / sizeof(components[0]); i++) {
		float x = from_bits(components[i][0]);
		float y = from_bits(components[i][1]);
		float z = from_bits(components[i][2]);
		struct vec2 input;
		vec2_set(&input, x, y);
		unsigned before_vertex = vertex_calls;
		unsigned before_texcoord = texcoord_calls;
		immediate_probe_vertex3f(x, y, z);
		assert_int_equal(vertex_calls, before_vertex + 1);
		assert_int_equal(texcoord_calls, before_texcoord);
		vertex_bits(&recorded_vertex, components[i][0], components[i][1], components[i][2]);
		immediate_probe_vertex2v(&input);
		assert_int_equal(vertex_calls, before_vertex + 2);
		assert_int_equal(texcoord_calls, before_texcoord);
		vertex_bits(&recorded_vertex, components[i][0], components[i][1], 0);
		for (unsigned j = 0; j < sizeof(units) / sizeof(units[0]); j++) {
			immediate_probe_texcoord(x, y, units[j]);
			assert_int_equal(texcoord_calls, before_texcoord + j + 1);
			assert_int_equal(recorded_unit, units[j]);
			same_bits(recorded_texcoord.x, components[i][0]);
			same_bits(recorded_texcoord.y, components[i][1]);
			assert_int_equal(vertex_calls, before_vertex + 2);
		}
		assert_int_equal(logs, 0);
	}
}

static void immediate_limits(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	graphics.using_immediate = true;
	const struct vec2 input = {{.x = -0.0f, .y = 2.5f}};
	/* Unit zero is the shared immediate-mode domain. The pre-existing immediate
	 * helper intentionally differs between C reference and hybrid for other units. */
	for (size_t i = 0; i < GS_IMMEDIATE_COUNT; i++) {
		if (i % 2)
			gs_vertex2v(&input);
		else
			gs_vertex3f(input.x, input.y, -0.0f);
		assert_int_equal(graphics.verts.num, i + 1);
		vertex_bits(&graphics.verts.array[i], 0x80000000u, 0x40200000u, i % 2 ? 0 : 0x80000000u);
		gs_texcoord(input.x, input.y, 0);
		assert_int_equal(graphics.texverts[0].num, i + 1);
		same_bits(graphics.texverts[0].array[i].x, 0x80000000u);
		same_bits(graphics.texverts[0].array[i].y, 0x40200000u);
	}
	gs_vertex3f(1, 2, 3);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_ERROR);
	assert_string_equal(last_log, "gs_vertex: tried to use over 512 for immediate rendering");
	gs_vertex2v(&input);
	assert_int_equal(logs, 2);
	assert_string_equal(last_log, "gs_vertex: tried to use over 512 for immediate rendering");
	gs_texcoord(1, 2, 0);
	assert_int_equal(logs, 3);
	assert_string_equal(last_log, "gs_texcoord: tried to use over 512 for immediate rendering");
	assert_int_equal(graphics.verts.num, GS_IMMEDIATE_COUNT);
	assert_int_equal(graphics.texverts[0].num, GS_IMMEDIATE_COUNT);
	/* The same counts are permitted when immediate mode is off. */
	graphics.using_immediate = false;
	gs_vertex3f(1, 2, 3);
	gs_vertex2v(&input);
	gs_texcoord(1, 2, 0);
	assert_int_equal(graphics.verts.num, GS_IMMEDIATE_COUNT + 2);
	assert_int_equal(graphics.texverts[0].num, GS_IMMEDIATE_COUNT + 1);
	assert_int_equal(logs, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(missing_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(argument_forwarding, setup, teardown),
					   cmocka_unit_test_setup_teardown(immediate_limits, setup, teardown),
					   cmocka_unit_test_setup_teardown(exact_downstream_calls, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
