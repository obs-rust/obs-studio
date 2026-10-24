#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-scene-item-names.h"
#include "util/dstr.h"

/*
 * Characterization of the scene item names and coordinate keys of
 * scene_load_item() and scene_save_item() in libobs/obs-scene.c, moved to
 * libobs/obs-scene-item-names.c before their Rust port:
 * - the value a saved scale filter, blend method or blend type name loads
 *   as (astrcmpi(), with Disable, Default and Normal for anything else), and
 *   which names the compare is asked about, in which order;
 * - the name each value is saved under ("disable", "default" and "normal"
 *   for anything else);
 * - which keys the position and scale, and the bounds, load from, and
 *   whether the relative keys are saved.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_scene_blend,
 * test_scene_item_crop_load (#1840), test_scene_duplicate_scale_ref and
 * test_saved_transition cover the caller through the real libobs.
 */

/* enum obs_scale_type, enum obs_blending_method and enum obs_blending_type
 * (obs.h); obs-scene.c checks the header's values against them */
#define SCALE_DISABLE 0
#define SCALE_POINT 1
#define SCALE_BICUBIC 2
#define SCALE_BILINEAR 3
#define SCALE_LANCZOS 4
#define SCALE_AREA 5
#define METHOD_DEFAULT 0
#define METHOD_SRGB_OFF 1
#define BLEND_NORMAL 0
#define BLEND_ADDITIVE 1
#define BLEND_SUBTRACT 2
#define BLEND_SCREEN 3
#define BLEND_MULTIPLY 4
#define BLEND_LIGHTEN 5
#define BLEND_DARKEN 6

#define KEYS_RELATIVE OBS_SCENE_ITEM_KEYS_RELATIVE
#define KEYS_FROM_ABSOLUTE OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE
#define KEYS_ABSOLUTE OBS_SCENE_ITEM_KEYS_ABSOLUTE

struct name_value {
	const char *name;
	int value;
};

/* Each name the loader knows, in lower, upper and mixed case. */
static const struct name_value scale_filter_names[] = {
	{"point", SCALE_POINT},       {"POINT", SCALE_POINT},       {"PoInT", SCALE_POINT},
	{"bilinear", SCALE_BILINEAR}, {"BILINEAR", SCALE_BILINEAR}, {"BiLinear", SCALE_BILINEAR},
	{"bicubic", SCALE_BICUBIC},   {"BICUBIC", SCALE_BICUBIC},   {"BiCubic", SCALE_BICUBIC},
	{"lanczos", SCALE_LANCZOS},   {"LANCZOS", SCALE_LANCZOS},   {"Lanczos", SCALE_LANCZOS},
	{"area", SCALE_AREA},         {"AREA", SCALE_AREA},         {"aReA", SCALE_AREA},
};

static const struct name_value blend_method_names[] = {
	{"srgb_off", METHOD_SRGB_OFF},
	{"SRGB_OFF", METHOD_SRGB_OFF},
	{"sRGB_Off", METHOD_SRGB_OFF},
};

static const struct name_value blend_type_names[] = {
	{"additive", BLEND_ADDITIVE}, {"ADDITIVE", BLEND_ADDITIVE}, {"Additive", BLEND_ADDITIVE},
	{"subtract", BLEND_SUBTRACT}, {"SUBTRACT", BLEND_SUBTRACT}, {"SubTract", BLEND_SUBTRACT},
	{"screen", BLEND_SCREEN},     {"SCREEN", BLEND_SCREEN},     {"sCrEeN", BLEND_SCREEN},
	{"multiply", BLEND_MULTIPLY}, {"MULTIPLY", BLEND_MULTIPLY}, {"Multiply", BLEND_MULTIPLY},
	{"lighten", BLEND_LIGHTEN},   {"LIGHTEN", BLEND_LIGHTEN},   {"LighTen", BLEND_LIGHTEN},
	{"darken", BLEND_DARKEN},     {"DARKEN", BLEND_DARKEN},     {"darKEN", BLEND_DARKEN},
};

/* Names no loader knows: they load as the default. "disable", "default" and
 * "normal" are what the saver writes for the default, and load as it by
 * falling through. */
static const char *const unknown_names[] = {
	"",
	"unknown",
	"point ",
	" point",
	"poin",
	"pointt",
	"bi linear",
	"srgb_off ",
	"srgb-off",
	"srgboff",
	"darken\t",
	"darken\n",
	"normal",
	"default",
	"disable",
	"NORMAL",
	"none",
	/* bytes above 0x7F */
	"\x80",
	"\xff",
	"po\xc3\xafnt",
	"\xc3\xa0rea",
	"area\xc2\xa0",
	"darken\xff",
	"\xe2\x80\x8b"
	"darken",
	"SRGB_OFF\x80",
	/* a dotted capital I (U+0130) and a dotless small i (U+0131) */
	"L\xc4\xb0GHTEN",
	"l\xc4\xb1ghten",
};

typedef int (*from_name_fn)(const char *name, obs_scene_item_name_cmp cmp);

static void check_names(const char *what, from_name_fn from_name, const struct name_value *names, size_t count)
{
	for (size_t i = 0; i < count; i++) {
		int got = from_name(names[i].name, astrcmpi);
		if (got != names[i].value) {
			fprintf(stderr, "%s \"%s\": %d, expected %d\n", what, names[i].name, got, names[i].value);
			fail();
		}
	}
}

static void check_unknown(const char *what, from_name_fn from_name, int expected)
{
	for (size_t i = 0; i < sizeof(unknown_names) / sizeof(unknown_names[0]); i++) {
		int got = from_name(unknown_names[i], astrcmpi);
		if (got != expected) {
			fprintf(stderr, "%s unknown #%zu: %d, expected %d\n", what, i, got, expected);
			fail();
		}
	}
	/* obs_data_get_string() never gives NULL, but the C checks for it */
	assert_int_equal(from_name(NULL, astrcmpi), expected);
}

/* ------------------------------------------------------------------------ */
/* Load: names */

static void scale_filter_from_name(void **state)
{
	(void)state;
	check_names("scale filter", obs_scene_item_scale_filter_from_name, scale_filter_names,
		    sizeof(scale_filter_names) / sizeof(scale_filter_names[0]));
}

static void blend_method_from_name(void **state)
{
	(void)state;
	check_names("blend method", obs_scene_item_blend_method_from_name, blend_method_names,
		    sizeof(blend_method_names) / sizeof(blend_method_names[0]));
}

static void blend_type_from_name(void **state)
{
	(void)state;
	check_names("blend type", obs_scene_item_blend_type_from_name, blend_type_names,
		    sizeof(blend_type_names) / sizeof(blend_type_names[0]));
}

static void unknown_names_load_as_default(void **state)
{
	(void)state;
	check_unknown("scale filter", obs_scene_item_scale_filter_from_name, SCALE_DISABLE);
	check_unknown("blend method", obs_scene_item_blend_method_from_name, METHOD_DEFAULT);
	check_unknown("blend type", obs_scene_item_blend_type_from_name, BLEND_NORMAL);
}

static void names_of_other_tables(void **state)
{
	(void)state;
	/* each table knows only its own names */
	assert_int_equal(obs_scene_item_scale_filter_from_name("darken", astrcmpi), SCALE_DISABLE);
	assert_int_equal(obs_scene_item_scale_filter_from_name("srgb_off", astrcmpi), SCALE_DISABLE);
	assert_int_equal(obs_scene_item_blend_method_from_name("point", astrcmpi), METHOD_DEFAULT);
	assert_int_equal(obs_scene_item_blend_method_from_name("additive", astrcmpi), METHOD_DEFAULT);
	assert_int_equal(obs_scene_item_blend_type_from_name("area", astrcmpi), BLEND_NORMAL);
	assert_int_equal(obs_scene_item_blend_type_from_name("srgb_off", astrcmpi), BLEND_NORMAL);
	/* characterized, not endorsed: numbers are names, not values */
	assert_int_equal(obs_scene_item_scale_filter_from_name("1", astrcmpi), SCALE_DISABLE);
	assert_int_equal(obs_scene_item_blend_type_from_name("6", astrcmpi), BLEND_NORMAL);
}

/* The compare's calls: each known name asked, in order, as (name, known). */
static const char *calls_name[16];
static const char *calls_known[16];
static size_t calls;

static int record_cmp(const char *name, const char *known)
{
	if (calls < 16) {
		calls_name[calls] = name;
		calls_known[calls] = known;
	}
	calls++;
	return astrcmpi(name, known);
}

static void check_calls(from_name_fn from_name, const char *name, const char *const *want, size_t count)
{
	calls = 0;
	from_name(name, record_cmp);
	if (calls != count) {
		fprintf(stderr, "\"%s\": %zu compares, expected %zu\n", name ? name : "(null)", calls, count);
		fail();
	}
	for (size_t i = 0; i < count; i++) {
		assert_ptr_equal(calls_name[i], name);
		assert_string_equal(calls_known[i], want[i]);
	}
}

static void compare_order(void **state)
{
	(void)state;
	static const char *const scale[] = {"point", "bilinear", "bicubic", "lanczos", "area"};
	static const char *const method[] = {"srgb_off"};
	static const char *const blend[] = {"additive", "subtract", "screen", "multiply", "lighten", "darken"};

	/* stops at the first match */
	check_calls(obs_scene_item_scale_filter_from_name, "POINT", scale, 1);
	check_calls(obs_scene_item_scale_filter_from_name, "bicubic", scale, 3);
	check_calls(obs_scene_item_blend_method_from_name, "srgb_off", method, 1);
	check_calls(obs_scene_item_blend_type_from_name, "screen", blend, 3);
	check_calls(obs_scene_item_blend_type_from_name, "Darken", blend, 6);
	/* every known name, then the default */
	check_calls(obs_scene_item_scale_filter_from_name, "disable", scale, 5);
	check_calls(obs_scene_item_blend_method_from_name, "default", method, 1);
	check_calls(obs_scene_item_blend_type_from_name, "normal", blend, 6);
	check_calls(obs_scene_item_blend_type_from_name, "", blend, 6);
	/* NULL: no compare */
	check_calls(obs_scene_item_scale_filter_from_name, NULL, scale, 0);
	check_calls(obs_scene_item_blend_method_from_name, NULL, method, 0);
	check_calls(obs_scene_item_blend_type_from_name, NULL, blend, 0);
}

/* ------------------------------------------------------------------------ */
/* Save: names */

static void scale_filter_names_saved(void **state)
{
	(void)state;
	assert_string_equal(obs_scene_item_scale_filter_name(SCALE_DISABLE), "disable");
	assert_string_equal(obs_scene_item_scale_filter_name(SCALE_POINT), "point");
	assert_string_equal(obs_scene_item_scale_filter_name(SCALE_BICUBIC), "bicubic");
	assert_string_equal(obs_scene_item_scale_filter_name(SCALE_BILINEAR), "bilinear");
	assert_string_equal(obs_scene_item_scale_filter_name(SCALE_LANCZOS), "lanczos");
	assert_string_equal(obs_scene_item_scale_filter_name(SCALE_AREA), "area");
	/* outside the enum */
	assert_string_equal(obs_scene_item_scale_filter_name(-1), "disable");
	assert_string_equal(obs_scene_item_scale_filter_name(6), "disable");
	assert_string_equal(obs_scene_item_scale_filter_name(INT_MIN), "disable");
	assert_string_equal(obs_scene_item_scale_filter_name(INT_MAX), "disable");
}

static void blend_method_names_saved(void **state)
{
	(void)state;
	assert_string_equal(obs_scene_item_blend_method_name(METHOD_DEFAULT), "default");
	assert_string_equal(obs_scene_item_blend_method_name(METHOD_SRGB_OFF), "srgb_off");
	/* outside the enum */
	assert_string_equal(obs_scene_item_blend_method_name(-1), "default");
	assert_string_equal(obs_scene_item_blend_method_name(2), "default");
	assert_string_equal(obs_scene_item_blend_method_name(INT_MIN), "default");
	assert_string_equal(obs_scene_item_blend_method_name(INT_MAX), "default");
}

static void blend_type_names_saved(void **state)
{
	(void)state;
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_NORMAL), "normal");
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_ADDITIVE), "additive");
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_SUBTRACT), "subtract");
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_SCREEN), "screen");
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_MULTIPLY), "multiply");
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_LIGHTEN), "lighten");
	assert_string_equal(obs_scene_item_blend_type_name(BLEND_DARKEN), "darken");
	/* outside the enum */
	assert_string_equal(obs_scene_item_blend_type_name(-1), "normal");
	assert_string_equal(obs_scene_item_blend_type_name(7), "normal");
	assert_string_equal(obs_scene_item_blend_type_name(INT_MIN), "normal");
	assert_string_equal(obs_scene_item_blend_type_name(INT_MAX), "normal");
}

static void saved_names_load_back(void **state)
{
	(void)state;
	/* every value in each enum, and the fallback, round-trips */
	for (int v = -2; v <= 8; v++) {
		int want = (v >= SCALE_DISABLE && v <= SCALE_AREA) ? v : SCALE_DISABLE;
		int got = obs_scene_item_scale_filter_from_name(obs_scene_item_scale_filter_name(v), astrcmpi);
		if (got != want) {
			fprintf(stderr, "scale filter %d: loads back as %d, expected %d\n", v, got, want);
			fail();
		}
		want = (v >= METHOD_DEFAULT && v <= METHOD_SRGB_OFF) ? v : METHOD_DEFAULT;
		got = obs_scene_item_blend_method_from_name(obs_scene_item_blend_method_name(v), astrcmpi);
		if (got != want) {
			fprintf(stderr, "blend method %d: loads back as %d, expected %d\n", v, got, want);
			fail();
		}
		want = (v >= BLEND_NORMAL && v <= BLEND_DARKEN) ? v : BLEND_NORMAL;
		got = obs_scene_item_blend_type_from_name(obs_scene_item_blend_type_name(v), astrcmpi);
		if (got != want) {
			fprintf(stderr, "blend type %d: loads back as %d, expected %d\n", v, got, want);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* Coordinate keys */

static void transform_keys(void **state)
{
	(void)state;
	/* all 16 patterns of the four keys, with and without absolute
	 * coordinates: pos_rel, scale_rel, scale_ref, bounds_rel */
	for (int abs = 0; abs < 2; abs++) {
		for (int flags = 0; flags < 16; flags++) {
			bool pos_rel = flags & 1;
			bool scale_rel = flags & 2;
			bool scale_ref = flags & 4;
			int want = abs                                   ? KEYS_ABSOLUTE
				   : (pos_rel && scale_rel && scale_ref) ? KEYS_RELATIVE
									 : KEYS_FROM_ABSOLUTE;
			int got = (int)obs_scene_item_transform_keys(abs, pos_rel, scale_rel, scale_ref);
			if (got != want) {
				fprintf(stderr, "transform abs %d flags %d: %d, expected %d\n", abs, flags, got, want);
				fail();
			}
		}
	}
}

static void bounds_keys(void **state)
{
	(void)state;
	for (int abs = 0; abs < 2; abs++) {
		for (int flags = 0; flags < 16; flags++) {
			bool bounds_rel = flags & 8;
			int want = abs ? KEYS_ABSOLUTE : bounds_rel ? KEYS_RELATIVE : KEYS_FROM_ABSOLUTE;
			int got = (int)obs_scene_item_bounds_keys(abs, bounds_rel);
			if (got != want) {
				fprintf(stderr, "bounds abs %d flags %d: %d, expected %d\n", abs, flags, got, want);
				fail();
			}
		}
	}
	/* characterized, not endorsed: the bounds load from bounds_rel without
	 * the transform's three keys, and the transform ignores bounds_rel */
	assert_int_equal(obs_scene_item_bounds_keys(false, true), KEYS_RELATIVE);
	assert_int_equal(obs_scene_item_transform_keys(false, false, false, false), KEYS_FROM_ABSOLUTE);
}

static void save_relative(void **state)
{
	(void)state;
	assert_true(obs_scene_item_save_relative(false));
	assert_false(obs_scene_item_save_relative(true));
}

static void key_values(void **state)
{
	(void)state;
	assert_int_equal(KEYS_RELATIVE, 0);
	assert_int_equal(KEYS_FROM_ABSOLUTE, 1);
	assert_int_equal(KEYS_ABSOLUTE, 2);
	assert_int_equal(sizeof(enum obs_scene_item_keys), sizeof(int));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scale_filter_from_name),
		cmocka_unit_test(blend_method_from_name),
		cmocka_unit_test(blend_type_from_name),
		cmocka_unit_test(unknown_names_load_as_default),
		cmocka_unit_test(names_of_other_tables),
		cmocka_unit_test(compare_order),
		cmocka_unit_test(scale_filter_names_saved),
		cmocka_unit_test(blend_method_names_saved),
		cmocka_unit_test(blend_type_names_saved),
		cmocka_unit_test(saved_names_load_back),
		cmocka_unit_test(transform_keys),
		cmocka_unit_test(bounds_keys),
		cmocka_unit_test(save_relative),
		cmocka_unit_test(key_values),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
