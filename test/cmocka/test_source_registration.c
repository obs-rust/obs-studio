/* Characterization of obs_register_source_s() in libobs/obs-module.c.h
 * (obs-rust/obs-studio#1876, slice 4): which list a source type goes in, the
 * size limit, the output flag rules, the composite checks, the required
 * callbacks, the versioned id, the log messages and their order, and the
 * type data freed on a failure. It pins the CURRENT behaviour before the
 * Rust port, through the public API only (obs_register_source_s(),
 * obs_enum_*_types(), obs_enum_input_types2(), obs_get_source_output_flags(),
 * obs_source_get_display_name() and the log handler), so the same test runs
 * on libobs with the C backend, the hybrid libobs and libobs-c-reference.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The checks run in this order, and the first that fails is the only one
 * logged: the type, a duplicate (id and version), the size, then (after the
 * flags are normalized) the composite checks and the required callbacks.
 * Every case registers its own ids, because a registration is never undone.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. It skips only when no display is set at all; with one set, a failed
 * obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>

/* ------------------------------------------------------------ log capture */

#define LOG_MAX 8

struct logged {
	int level;
	char msg[512];
};

static struct logged logged[LOG_MAX];
static size_t logged_num;
static bool capturing;
static log_handler_t prev_handler;
static void *prev_param;

/* Only the registration's messages are kept: they all name
 * obs_register_source. Anything else goes to the previous handler. */
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (capturing && strstr(format, "obs_register_source")) {
		if (logged_num < LOG_MAX) {
			logged[logged_num].level = level;
			vsnprintf(logged[logged_num].msg, sizeof(logged[0].msg), format, args);
		}
		logged_num++;
		return;
	}
	if (prev_handler)
		prev_handler(level, format, args, prev_param);
}

/* --------------------------------------------------------------- callbacks */

static char type_data_tag[] = "Registered";
static int frees;
static void *freed;

static const char *get_name(void *type_data)
{
	return type_data ? (const char *)type_data : "(no type data)";
}

static uint32_t get_size(void *data)
{
	(void)data;
	return 16;
}

static bool audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio_output, uint32_t mixers,
			 size_t channels, size_t sample_rate)
{
	(void)data;
	(void)ts_out;
	(void)audio_output;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void free_type_data(void *type_data)
{
	frees++;
	freed = type_data;
}

/* A source type with every callback the registration checks, its type data
 * and free_type_data. */
static struct obs_source_info full_info(const char *id, enum obs_source_type type, uint32_t flags)
{
	struct obs_source_info info = {0};
	info.id = id;
	info.type = type;
	info.output_flags = flags;
	info.get_name = get_name;
	info.get_width = get_size;
	info.get_height = get_size;
	info.audio_render = audio_render;
	info.type_data = type_data_tag;
	info.free_type_data = free_type_data;
	return info;
}

/* Room after the structure, for a size above sizeof(struct
 * obs_source_info). */
struct padded_info {
	struct obs_source_info info;
	unsigned char after[64];
};

static void reg(const struct obs_source_info *info, size_t size)
{
	struct padded_info padded;
	memset(&padded, 0xA5, sizeof(padded));
	padded.info = *info;

	logged_num = 0;
	frees = 0;
	freed = NULL;
	capturing = true;
	obs_register_source_s(&padded.info, size);
	capturing = false;
}

/* ----------------------------------------------------------- expectations */

static void expect_logged(size_t num, ...)
{
	va_list args;
	va_start(args, num);
	if (logged_num != num) {
		for (size_t i = 0; i < logged_num && i < LOG_MAX; i++)
			fprintf(stderr, "logged %zu: %s\n", i, logged[i].msg);
	}
	assert_int_equal(logged_num, num);
	for (size_t i = 0; i < num; i++) {
		int level = va_arg(args, int);
		const char *msg = va_arg(args, const char *);
		assert_string_equal(logged[i].msg, msg);
		assert_int_equal(logged[i].level, level);
	}
	va_end(args);
}

static bool listed(bool (*enum_types)(size_t, const char **), const char *id, const char **found)
{
	const char *t;
	for (size_t i = 0; enum_types(i, &t); i++) {
		if (strcmp(t, id) == 0) {
			if (found)
				*found = t;
			return true;
		}
	}
	return false;
}

static size_t count(bool (*enum_types)(size_t, const char **))
{
	const char *t;
	size_t i = 0;
	while (enum_types(i, &t))
		i++;
	return i;
}

enum list { IN_NONE, IN_INPUT, IN_FILTER, IN_TRANSITION };

/* id is registered, in source_types and in exactly the list given, with
 * these output flags, and its display name comes from get_name(). */
static void expect_registered(const char *id, enum list list, uint32_t flags)
{
	assert_true(listed(obs_enum_source_types, id, NULL));
	assert_int_equal(listed(obs_enum_input_types, id, NULL), list == IN_INPUT);
	assert_int_equal(listed(obs_enum_filter_types, id, NULL), list == IN_FILTER);
	assert_int_equal(listed(obs_enum_transition_types, id, NULL), list == IN_TRANSITION);
	assert_int_equal(obs_get_source_output_flags(id), flags);
	assert_non_null(obs_source_get_display_name(id));
	assert_string_equal(obs_source_get_display_name(id), type_data_tag);
}

static void expect_absent(const char *id)
{
	assert_false(listed(obs_enum_source_types, id, NULL));
	assert_false(listed(obs_enum_input_types, id, NULL));
	assert_false(listed(obs_enum_filter_types, id, NULL));
	assert_false(listed(obs_enum_transition_types, id, NULL));
	assert_int_equal(obs_get_source_output_flags(id), 0);
	assert_null(obs_source_get_display_name(id));
}

/* A failed registration: nothing added anywhere, and the type data freed
 * when the copied part of the structure holds both. */
static void expect_rejected(const char *id, size_t source_types, bool type_data_freed)
{
	expect_absent(id);
	assert_int_equal(count(obs_enum_source_types), source_types);
	assert_int_equal(frees, type_data_freed ? 1 : 0);
	if (type_data_freed)
		assert_ptr_equal(freed, type_data_tag);
}

#define W LOG_WARNING
#define E LOG_ERROR
#define REQUIRED(val, id) "Required value '" val "' for '" id "' not found.  obs_register_source failed."
#define COMPOSITE_AUDIO(id) "obs_register_source: Source '" id "': Composite sources cannot be audio sources"
#define COMPOSITE_ASYNC(id) "obs_register_source: Source '" id "': Composite sources cannot be async sources"

/* ------------------------------------------------------------------ setup */

static bool obs_up;

static int group_setup(void **state)
{
	(void)state;
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	base_set_log_handler(prev_handler, prev_param);
	return 0;
}

#define REQUIRE_OBS()           \
	do {                    \
		if (!obs_up)    \
			skip(); \
	} while (false)

/* ------------------------------------------------------- type and lists */

static void test_type_lists(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_source_info info = full_info("sr_input", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_input", IN_INPUT, OBS_SOURCE_VIDEO);
	assert_int_equal(frees, 0);

	info = full_info("sr_filter", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_filter", IN_FILTER, OBS_SOURCE_VIDEO);

	info = full_info("sr_transition", OBS_SOURCE_TYPE_TRANSITION, 0);
	info.get_width = NULL;
	info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_transition", IN_TRANSITION,
			  OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW);

	/* A scene type goes in no list of its own: only in source_types. */
	size_t inputs = count(obs_enum_input_types);
	size_t filters = count(obs_enum_filter_types);
	size_t transitions = count(obs_enum_transition_types);
	info = full_info("sr_scene", OBS_SOURCE_TYPE_SCENE, OBS_SOURCE_VIDEO | OBS_SOURCE_COMPOSITE);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_scene", IN_NONE, OBS_SOURCE_VIDEO | OBS_SOURCE_COMPOSITE);
	assert_int_equal(count(obs_enum_input_types), inputs);
	assert_int_equal(count(obs_enum_filter_types), filters);
	assert_int_equal(count(obs_enum_transition_types), transitions);
}

static void test_unknown_type(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = count(obs_enum_source_types);

	struct obs_source_info info = full_info("sr_type4", (enum obs_source_type)4, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(1, W, "obs_register_source: Tried to register unknown source type: 4");
	expect_rejected("sr_type4", types, true);

	info = full_info("sr_type_max", (enum obs_source_type)UINT32_MAX, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(1, W, "obs_register_source: Tried to register unknown source type: 4294967295");
	expect_rejected("sr_type_max", types, true);

	/* The type is checked first: before a duplicate, the size and the
	 * callbacks. */
	info = full_info("sr_input", (enum obs_source_type)7, OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO);
	info.get_name = NULL;
	reg(&info, sizeof(info) + 8);
	expect_logged(1, W, "obs_register_source: Tried to register unknown source type: 7");
	assert_int_equal(count(obs_enum_source_types), types);
	assert_int_equal(frees, 1);

	/* With a size of 0, nothing is copied, so nothing is freed. */
	info = full_info("sr_type9", (enum obs_source_type)9, OBS_SOURCE_VIDEO);
	reg(&info, 0);
	expect_logged(1, W, "obs_register_source: Tried to register unknown source type: 9");
	expect_rejected("sr_type9", types, false);
}

/* ------------------------------------------------------------ duplicates */

static void test_duplicate(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_source_info info = full_info("sr_dup", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	reg(&info, sizeof(info));
	expect_logged(0);
	size_t types = count(obs_enum_source_types);

	/* The same id and version again, of any type. */
	info = full_info("sr_dup", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(1, W, "obs_register_source: Source 'sr_dup' already exists!  Duplicate library?");
	assert_int_equal(count(obs_enum_source_types), types);
	assert_int_equal(frees, 1);
	assert_int_equal(obs_get_source_output_flags("sr_dup"), OBS_SOURCE_AUDIO);

	/* A duplicate is found before the size and the callbacks are checked. */
	info.get_name = NULL;
	reg(&info, sizeof(info) + 8);
	expect_logged(1, W, "obs_register_source: Source 'sr_dup' already exists!  Duplicate library?");
	assert_int_equal(count(obs_enum_source_types), types);

	/* Another version of the same id is not a duplicate. */
	info = full_info("sr_dup", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	info.version = 2;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_dup_v2", IN_INPUT, OBS_SOURCE_AUDIO);
	reg(&info, sizeof(info));
	expect_logged(1, W, "obs_register_source: Source 'sr_dup' already exists!  Duplicate library?");
}

/* ------------------------------------------------------------------ size */

static void test_size(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = count(obs_enum_source_types);
	char msg[256];
	snprintf(msg, sizeof(msg),
		 "obs_register_source: Tried to register obs_source_info with size %llu which is more than libobs "
		 "currently supports (%llu)",
		 (long long unsigned)sizeof(struct obs_source_info) + 1,
		 (long long unsigned)sizeof(struct obs_source_info));

	/* One byte more than libobs knows: rejected, the type data freed. */
	struct obs_source_info info = full_info("sr_size_over", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info) + 1);
	expect_logged(1, W, msg);
	expect_rejected("sr_size_over", types, true);

	/* The size is checked before the flags and the callbacks. */
	info = full_info("sr_size_over2", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO);
	info.get_name = NULL;
	reg(&info, sizeof(info) + 1);
	expect_logged(1, W, msg);
	expect_rejected("sr_size_over2", types, true);

	/* Exactly the size libobs knows. */
	info = full_info("sr_size_exact", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_size_exact", IN_INPUT, OBS_SOURCE_VIDEO);
}

/* A smaller size: only that much of the structure is copied, and a callback
 * counts only when all of it is inside the size. */
static void test_short_size(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = count(obs_enum_source_types);

	/* Up to and with get_name only: an audio input needs nothing more. */
	struct obs_source_info info = full_info("sr_short_name", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	reg(&info, offsetof(struct obs_source_info, get_name) + sizeof(info.get_name));
	expect_logged(0);
	assert_true(listed(obs_enum_input_types, "sr_short_name", NULL));
	assert_int_equal(obs_get_source_output_flags("sr_short_name"), OBS_SOURCE_AUDIO);
	/* The type data was not copied. */
	assert_string_equal(obs_source_get_display_name("sr_short_name"), "(no type data)");
	types++;

	/* get_name cut off, whole or in part. */
	info = full_info("sr_short_noname", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	reg(&info, offsetof(struct obs_source_info, get_name));
	expect_logged(1, E, REQUIRED("get_name", "sr_short_noname"));
	expect_rejected("sr_short_noname", types, false);
	reg(&info, offsetof(struct obs_source_info, get_name) + sizeof(info.get_name) - 1);
	expect_logged(1, E, REQUIRED("get_name", "sr_short_noname"));
	expect_rejected("sr_short_noname", types, false);

	/* A sync video input cut before get_width, or before get_height. */
	info = full_info("sr_short_width", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	reg(&info, offsetof(struct obs_source_info, get_width));
	expect_logged(1, E, REQUIRED("get_width", "sr_short_width"));
	expect_rejected("sr_short_width", types, false);
	info = full_info("sr_short_height", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	reg(&info, offsetof(struct obs_source_info, get_height));
	expect_logged(1, E, REQUIRED("get_height", "sr_short_height"));
	expect_rejected("sr_short_height", types, false);

	/* type_data inside the size but free_type_data not: not freed. */
	info = full_info("sr_short_free", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	info.get_name = NULL;
	reg(&info, offsetof(struct obs_source_info, free_type_data));
	expect_logged(1, E, REQUIRED("get_name", "sr_short_free"));
	expect_rejected("sr_short_free", types, false);
	reg(&info, offsetof(struct obs_source_info, free_type_data) + sizeof(info.free_type_data));
	expect_logged(1, E, REQUIRED("get_name", "sr_short_free"));
	expect_rejected("sr_short_free", types, true);

	/* A composite source cut before audio_render. */
	info = full_info("sr_short_render", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO | OBS_SOURCE_COMPOSITE);
	reg(&info, offsetof(struct obs_source_info, audio_render));
	expect_logged(1, E, REQUIRED("audio_render", "sr_short_render"));
	expect_rejected("sr_short_render", types, true);

	/* Size 0: nothing copied, get_name missing, nothing freed. */
	info = full_info("sr_short_zero", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_VIDEO);
	reg(&info, 0);
	expect_logged(1, E, REQUIRED("get_name", "sr_short_zero"));
	expect_rejected("sr_short_zero", types, false);

	/* The flags are normalized and checked on the copy, so the composite
	 * checks see the flags only when they are inside the size, and they
	 * run before get_name is required. */
	info = full_info("sr_short_flags", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO);
	reg(&info, offsetof(struct obs_source_info, output_flags));
	expect_logged(1, E, REQUIRED("get_name", "sr_short_flags"));
	expect_rejected("sr_short_flags", types, false);
	reg(&info, offsetof(struct obs_source_info, output_flags) + sizeof(info.output_flags));
	expect_logged(1, W, COMPOSITE_AUDIO("sr_short_flags"));
	expect_rejected("sr_short_flags", types, false);

	/* The list is picked by the caller's type, the flags are normalized by
	 * the copy's. A filter cut before its type is an input with no flags
	 * on the copy, so get_name is what is missing. With the type and the
	 * flags copied, the composite filter without video is made async and
	 * rejected (characterized, not endorsed). */
	info = full_info("sr_short_type", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_COMPOSITE);
	reg(&info, offsetof(struct obs_source_info, type));
	expect_logged(1, E, REQUIRED("get_name", "sr_short_type"));
	expect_rejected("sr_short_type", types, false);
	reg(&info, offsetof(struct obs_source_info, get_name));
	expect_logged(1, W, COMPOSITE_ASYNC("sr_short_type"));
	expect_rejected("sr_short_type", types, false);

	/* Cut before version: the copy has version 0, so the id is not
	 * versioned, though the duplicate lookup used the caller's version
	 * (characterized, not endorsed). */
	info = full_info("sr_short_version", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	info.version = 3;
	reg(&info, offsetof(struct obs_source_info, version));
	expect_logged(0);
	expect_registered("sr_short_version", IN_INPUT, OBS_SOURCE_VIDEO);
	assert_false(listed(obs_enum_source_types, "sr_short_version_v3", NULL));
	/* So the full version 3 is a new type, and version 0 a duplicate. */
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_short_version_v3", IN_INPUT, OBS_SOURCE_VIDEO);
	info.version = 0;
	reg(&info, sizeof(info));
	expect_logged(1, W, "obs_register_source: Source 'sr_short_version' already exists!  Duplicate library?");
}

/* ----------------------------------------------------------------- flags */

static void test_filter_flags(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* An audio-only filter, or one with no flags, is made async. */
	struct obs_source_info info = full_info("sr_filter_audio", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_AUDIO);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_filter_audio", IN_FILTER, OBS_SOURCE_AUDIO | OBS_SOURCE_ASYNC);

	info = full_info("sr_filter_none", OBS_SOURCE_TYPE_FILTER, 0);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_filter_none", IN_FILTER, OBS_SOURCE_ASYNC);

	/* Other bits are kept. */
	info = full_info("sr_filter_bits", OBS_SOURCE_TYPE_FILTER,
			 OBS_SOURCE_AUDIO | OBS_SOURCE_DEPRECATED | (1u << 31));
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_filter_bits", IN_FILTER,
			  OBS_SOURCE_AUDIO | OBS_SOURCE_DEPRECATED | (1u << 31) | OBS_SOURCE_ASYNC);

	/* A video filter keeps its flags, async or not, and needs no size. */
	info = full_info("sr_filter_async_video", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_ASYNC_VIDEO);
	info.get_width = NULL;
	info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_filter_async_video", IN_FILTER, OBS_SOURCE_ASYNC_VIDEO);

	/* An input, a scene or a transition is not made async. */
	info = full_info("sr_input_audio", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	reg(&info, sizeof(info));
	expect_registered("sr_input_audio", IN_INPUT, OBS_SOURCE_AUDIO);
	info = full_info("sr_scene_none", OBS_SOURCE_TYPE_SCENE, 0);
	reg(&info, sizeof(info));
	expect_registered("sr_scene_none", IN_NONE, 0);
}

static void test_transition_flags(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = count(obs_enum_source_types);

	/* A transition always gets composite, video and custom draw, other bits
	 * kept; get_width and get_height are warned about and kept. */
	struct obs_source_info info =
		full_info("sr_trans_bits", OBS_SOURCE_TYPE_TRANSITION, OBS_SOURCE_DEPRECATED | OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(2, W, "obs_register_source: get_width ignored registering transition 'sr_trans_bits'", W,
		      "obs_register_source: get_height ignored registering transition 'sr_trans_bits'");
	expect_registered("sr_trans_bits", IN_TRANSITION,
			  OBS_SOURCE_DEPRECATED | OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW);

	info = full_info("sr_trans_height", OBS_SOURCE_TYPE_TRANSITION, 0);
	info.get_width = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, W, "obs_register_source: get_height ignored registering transition 'sr_trans_height'");
	types = count(obs_enum_source_types);

	/* Composite, so it needs audio_render. */
	info = full_info("sr_trans_norender", OBS_SOURCE_TYPE_TRANSITION, 0);
	info.get_width = info.get_height = NULL;
	info.audio_render = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("audio_render", "sr_trans_norender"));
	expect_rejected("sr_trans_norender", types, true);

	/* Composite, so it can be neither audio nor async. The warnings come
	 * first. */
	info = full_info("sr_trans_audio", OBS_SOURCE_TYPE_TRANSITION, OBS_SOURCE_AUDIO);
	reg(&info, sizeof(info));
	expect_logged(3, W, "obs_register_source: get_width ignored registering transition 'sr_trans_audio'", W,
		      "obs_register_source: get_height ignored registering transition 'sr_trans_audio'", W,
		      COMPOSITE_AUDIO("sr_trans_audio"));
	expect_rejected("sr_trans_audio", types, true);

	info = full_info("sr_trans_async", OBS_SOURCE_TYPE_TRANSITION, OBS_SOURCE_ASYNC);
	info.get_width = info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, W, COMPOSITE_ASYNC("sr_trans_async"));
	expect_rejected("sr_trans_async", types, true);
}

/* ------------------------------------------------------------- composite */

static void test_composite(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = count(obs_enum_source_types);

	struct obs_source_info info =
		full_info("sr_comp_audio", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO);
	reg(&info, sizeof(info));
	expect_logged(1, W, COMPOSITE_AUDIO("sr_comp_audio"));
	expect_rejected("sr_comp_audio", types, true);

	info = full_info("sr_comp_async", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_COMPOSITE | OBS_SOURCE_ASYNC_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(1, W, COMPOSITE_ASYNC("sr_comp_async"));
	expect_rejected("sr_comp_async", types, true);

	/* Audio is checked before async, and both before the callbacks. */
	info = full_info("sr_comp_both", OBS_SOURCE_TYPE_INPUT,
			 OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO | OBS_SOURCE_ASYNC);
	info.get_name = NULL;
	info.audio_render = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, W, COMPOSITE_AUDIO("sr_comp_both"));
	expect_rejected("sr_comp_both", types, true);

	/* A filter without video is made async first, so a composite one is
	 * rejected as async. */
	info = full_info("sr_comp_filter", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_COMPOSITE);
	reg(&info, sizeof(info));
	expect_logged(1, W, COMPOSITE_ASYNC("sr_comp_filter"));
	expect_rejected("sr_comp_filter", types, true);

	/* A composite source needs audio_render; once that is there, it is
	 * registered. */
	info = full_info("sr_comp_norender", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO);
	info.audio_render = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("audio_render", "sr_comp_norender"));
	expect_rejected("sr_comp_norender", types, true);

	info = full_info("sr_comp_filter_video", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO);
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_comp_filter_video", IN_FILTER, OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO);

	/* Not composite: audio_render is not needed. */
	info = full_info("sr_plain_norender", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	info.audio_render = NULL;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_plain_norender", IN_INPUT, OBS_SOURCE_AUDIO);
}

/* -------------------------------------------------------------- required */

static void test_required(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = count(obs_enum_source_types);

	struct obs_source_info info = full_info("sr_req_noname", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	info.get_name = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_name", "sr_req_noname"));
	expect_rejected("sr_req_noname", types, true);

	/* get_name first, then get_width, then get_height. */
	info = full_info("sr_req_none", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO);
	info.get_name = NULL;
	info.get_width = info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_name", "sr_req_none"));
	info.get_name = get_name;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_width", "sr_req_none"));
	info.get_width = get_size;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_height", "sr_req_none"));
	expect_rejected("sr_req_none", types, true);
	info.get_width = NULL;
	info.get_height = get_size;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_width", "sr_req_none"));

	/* get_width and get_height before audio_render. */
	info = full_info("sr_req_comp", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO | OBS_SOURCE_COMPOSITE);
	info.get_height = NULL;
	info.audio_render = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_height", "sr_req_comp"));

	/* A sync video scene type needs them too. */
	info = full_info("sr_req_scene", OBS_SOURCE_TYPE_SCENE, OBS_SOURCE_VIDEO);
	info.get_width = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_width", "sr_req_scene"));
	expect_rejected("sr_req_scene", types, true);

	/* Not needed: async video, no video, a filter, a transition. */
	info = full_info("sr_req_async", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_ASYNC_VIDEO);
	info.get_width = info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_req_async", IN_INPUT, OBS_SOURCE_ASYNC_VIDEO);

	info = full_info("sr_req_audio", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	info.get_width = info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_req_audio", IN_INPUT, OBS_SOURCE_AUDIO);

	info = full_info("sr_req_filter", OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_VIDEO);
	info.get_width = info.get_height = NULL;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered("sr_req_filter", IN_FILTER, OBS_SOURCE_VIDEO);

	/* Without free_type_data, or without type data, nothing is freed. */
	types = count(obs_enum_source_types);
	info = full_info("sr_req_nofree", OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	info.get_name = NULL;
	info.free_type_data = NULL;
	reg(&info, sizeof(info));
	expect_logged(1, E, REQUIRED("get_name", "sr_req_nofree"));
	expect_rejected("sr_req_nofree", types, false);
	info.free_type_data = free_type_data;
	info.type_data = NULL;
	reg(&info, sizeof(info));
	expect_rejected("sr_req_nofree", types, false);
}

/* ------------------------------------------------------------ versioned id */

static void expect_versioned(const char *id, uint32_t version, const char *expected)
{
	struct obs_source_info info = full_info(id, OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO);
	info.version = version;
	reg(&info, sizeof(info));
	expect_logged(0);
	expect_registered(expected, IN_INPUT, OBS_SOURCE_AUDIO);

	const char *t = NULL, *unversioned = NULL;
	bool found = false;
	for (size_t i = 0; obs_enum_input_types2(i, &t, &unversioned); i++) {
		if (strcmp(t, expected) == 0) {
			found = true;
			break;
		}
	}
	assert_true(found);
	assert_string_equal(unversioned, id);
	/* A copy, not the caller's string. */
	assert_ptr_not_equal(t, id);
}

static void test_versioned_id(void **state)
{
	(void)state;
	REQUIRE_OBS();

	expect_versioned("sr_ver", 0, "sr_ver");
	expect_versioned("sr_ver", 1, "sr_ver_v1");
	expect_versioned("sr_ver", 10, "sr_ver_v10");
	expect_versioned("sr_ver", 2147483647u, "sr_ver_v2147483647");
	/* The version is printed as an int (characterized, not endorsed). */
	expect_versioned("sr_ver", 2147483648u, "sr_ver_v-2147483648");
	expect_versioned("sr_ver", UINT32_MAX, "sr_ver_v-1");
	/* The id is not a format. */
	expect_versioned("sr_%s%d", 4, "sr_%s%d_v4");
	expect_versioned("", 5, "_v5");
	/* The unversioned id is the caller's pointer; the id with its version
	 * is a new type, though an id of that name may exist. */
	expect_versioned("sr_ver_v1", 1, "sr_ver_v1_v1");
	assert_non_null(obs_source_get_display_name("sr_ver_v1"));
}

#define CASE(fn) cmocka_unit_test(fn)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_type_lists), CASE(test_unknown_type), CASE(test_duplicate),        CASE(test_size),
		CASE(test_short_size), CASE(test_filter_flags), CASE(test_transition_flags), CASE(test_composite),
		CASE(test_required),   CASE(test_versioned_id),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
