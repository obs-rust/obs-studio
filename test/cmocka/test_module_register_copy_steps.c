/* Characterization of the registration size rules of libobs/obs-module.c.h
 * (obs-rust/obs-studio#2860), moved to libobs/obs-module-register-copy.c
 * before their port to rust/obs-core (module_register_copy). It pins the
 * CURRENT behaviour; cases marked "characterized, not endorsed" are odd but
 * real.
 *
 * Two parts:
 * - the steps: the four hidden functions of obs-module-register-copy.c,
 *   compiled into this test from the moved file: the copy length for every
 *   size around each field of a pointer-sized layout and a 32-bit one, the
 *   in-size rule, the zero and too-large gate and the transition rule;
 * - the callers: obs_register_source_s(), obs_register_output_s(),
 *   obs_register_encoder_s() and obs_register_service_s() of the real
 *   libobs, through the public API (the display names, the defaults, the
 *   icon type and the log handler), with sizes cut at a callback's end and
 *   one byte before it, inside a pointer after a 32-bit field, at 0 and
 *   past sizeof(). These run on libobs with the C backend, the hybrid
 *   libobs and libobs-c-reference.
 *
 * Every structure is registered from a buffer at least as large as libobs's
 * structure, so a cut never reads past the buffer: what is registered shows
 * what was copied. Every case registers its own ids, because a registration
 * is never undone.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. The caller cases skip only when no display is set at all; with one
 * set, a failed obs_startup() fails the test. */

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
#include <obs-module-register-copy.h>
#include <util/base.h>

/* ================================================================ steps */

static size_t whole(size_t size, size_t ptr, const size_t *u32, size_t count)
{
	return obs_module_register_copy_whole_size(size, ptr, u32, count);
}

/* No 32-bit field: the size rounded down to a pointer, for both pointer
 * sizes libobs builds for. */
static void test_whole_size_pointers_only(void **state)
{
	(void)state;
	for (size_t size = 0; size < 64; size++) {
		assert_int_equal(whole(size, 8, NULL, 0), size - size % 8);
		assert_int_equal(whole(size, 4, NULL, 0), size - size % 4);
	}
	assert_int_equal(whole(0, 8, NULL, 0), 0);
	assert_int_equal(whole(7, 8, NULL, 0), 0);
	assert_int_equal(whole(8, 8, NULL, 0), 8);
	assert_int_equal(whole(15, 8, NULL, 0), 8);
	assert_int_equal(whole(16, 8, NULL, 0), 16);
	assert_int_equal(whole(3, 4, NULL, 0), 0);
	assert_int_equal(whole(5, 4, NULL, 0), 4);
}

/* The source layout on a 64-bit build: id, then type and output_flags in
 * one pointer slot, then pointers. Each 32-bit field counts once it lies
 * wholly within the size, even inside a pointer slot. */
static void test_whole_size_source_head(void **state)
{
	(void)state;
	const size_t u32[] = {8, 12};

	assert_int_equal(whole(8, 8, u32, 2), 8);
	assert_int_equal(whole(9, 8, u32, 2), 8);
	assert_int_equal(whole(11, 8, u32, 2), 8);
	assert_int_equal(whole(12, 8, u32, 2), 12);
	assert_int_equal(whole(13, 8, u32, 2), 12);
	assert_int_equal(whole(15, 8, u32, 2), 12);
	assert_int_equal(whole(16, 8, u32, 2), 16);
	assert_int_equal(whole(23, 8, u32, 2), 16);
	assert_int_equal(whole(24, 8, u32, 2), 24);
}

/* A 32-bit field followed by padding and a pointer (icon_type and version
 * on a 64-bit build): a size inside the padding or the next pointer keeps
 * the field and zeroes the pointer. */
static void test_whole_size_padded_field(void **state)
{
	(void)state;
	const size_t u32[] = {288};

	assert_int_equal(whole(288, 8, u32, 1), 288);
	assert_int_equal(whole(289, 8, u32, 1), 288);
	assert_int_equal(whole(291, 8, u32, 1), 288);
	assert_int_equal(whole(292, 8, u32, 1), 292);
	assert_int_equal(whole(295, 8, u32, 1), 292);
	assert_int_equal(whole(296, 8, u32, 1), 296);
	assert_int_equal(whole(299, 8, u32, 1), 296);
	assert_int_equal(whole(303, 8, u32, 1), 296);
	assert_int_equal(whole(304, 8, u32, 1), 304);
}

/* The same fields on a 32-bit build, where a 32-bit field fills a pointer
 * slot: it never extends the copy past the rounded size. */
static void test_whole_size_32bit(void **state)
{
	(void)state;
	const size_t u32[] = {4, 8, 144, 184};

	for (size_t size = 0; size < 200; size++)
		assert_int_equal(whole(size, 4, u32, 4), size - size % 4);
}

/* A field before the rounded size, or one only partly within it, changes
 * nothing; two fields in one slot extend the copy in either list order. */
static void test_whole_size_order(void **state)
{
	(void)state;
	const size_t before[] = {0};
	const size_t forward[] = {16, 20};
	const size_t backward[] = {20, 16};
	const size_t overlap_forward[] = {16, 18};
	const size_t overlap_backward[] = {18, 16};

	assert_int_equal(whole(12, 8, before, 1), 8);
	assert_int_equal(whole(19, 8, forward, 2), 16);
	assert_int_equal(whole(20, 8, forward, 2), 20);
	assert_int_equal(whole(23, 8, forward, 2), 20);
	assert_int_equal(whole(20, 8, backward, 2), 20);
	assert_int_equal(whole(23, 8, backward, 2), 20);
	assert_int_equal(whole(24, 8, forward, 2), 24);
	/* characterized, not endorsed: the list is read once, in order, so
	 * with overlapping fields (no real layout has them) a field that
	 * starts before the copy's new end no longer counts. */
	assert_int_equal(whole(22, 8, overlap_forward, 2), 20);
	assert_int_equal(whole(22, 8, overlap_backward, 2), 22);
}

static void test_within(void **state)
{
	(void)state;
	assert_true(obs_module_register_copy_within(0, 0, 0));
	assert_true(obs_module_register_copy_within(0, 8, 8));
	assert_false(obs_module_register_copy_within(0, 8, 7));
	assert_true(obs_module_register_copy_within(16, 8, 24));
	assert_false(obs_module_register_copy_within(16, 8, 23));
	assert_true(obs_module_register_copy_within(16, 8, 1000));
	assert_true(obs_module_register_copy_within(368, 4, 372));
	assert_false(obs_module_register_copy_within(368, 4, 371));
	assert_false(obs_module_register_copy_within(368, 4, 0));
}

static void test_gate(void **state)
{
	(void)state;
	assert_int_equal(obs_module_register_copy_gate(0, 192), OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD);
	assert_int_equal(obs_module_register_copy_gate(0, 0), OBS_MODULE_REGISTER_COPY_OUTSIDE_LOAD);
	assert_int_equal(obs_module_register_copy_gate(1, 192), OBS_MODULE_REGISTER_COPY_COPY);
	assert_int_equal(obs_module_register_copy_gate(191, 192), OBS_MODULE_REGISTER_COPY_COPY);
	assert_int_equal(obs_module_register_copy_gate(192, 192), OBS_MODULE_REGISTER_COPY_COPY);
	assert_int_equal(obs_module_register_copy_gate(193, 192), OBS_MODULE_REGISTER_COPY_TOO_LARGE);
	assert_int_equal(obs_module_register_copy_gate(SIZE_MAX, 192), OBS_MODULE_REGISTER_COPY_TOO_LARGE);
	assert_int_equal(obs_module_register_copy_gate(1, 0), OBS_MODULE_REGISTER_COPY_TOO_LARGE);
}

static void test_ignores_size(void **state)
{
	(void)state;
	assert_true(obs_module_register_copy_ignores_size(OBS_SOURCE_TYPE_TRANSITION));
	assert_false(obs_module_register_copy_ignores_size(OBS_SOURCE_TYPE_INPUT));
	assert_false(obs_module_register_copy_ignores_size(OBS_SOURCE_TYPE_FILTER));
	assert_false(obs_module_register_copy_ignores_size(OBS_SOURCE_TYPE_SCENE));
	assert_false(obs_module_register_copy_ignores_size(4));
	assert_false(obs_module_register_copy_ignores_size(UINT32_MAX));
}

/* =============================================================== callers */

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

/* Only the registration's messages are kept: they name obs_register_* or
 * start "Tried to register". Anything else goes to the previous handler. */
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (capturing && (strstr(format, "obs_register_") || strstr(format, "Tried to register"))) {
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

#define W LOG_WARNING
#define E LOG_ERROR
#define REQUIRED(val, id, func) "Required value '" val "' for '" id "' not found.  " func " failed."

/* --------------------------------------------------------------- callbacks */

static char type_data_tag[] = "Registered";
static int frees;
static int defaults_calls;

static void free_type_data(void *type_data)
{
	(void)type_data;
	frees++;
}

static void get_defaults(obs_data_t *settings)
{
	(void)settings;
	defaults_calls++;
}

static const char *get_name(void *type_data)
{
	(void)type_data;
	return "mrc";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return NULL;
}

static void destroy(void *data)
{
	(void)data;
}

static uint32_t get_size(void *data)
{
	(void)data;
	return 0;
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

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return NULL;
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return NULL;
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received_packet;
	return false;
}

static void *service_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	return NULL;
}

static const char *get_protocol(void *data)
{
	(void)data;
	return "RTMP";
}

/* --------------------------------------------------------- the structures */

/* Each structure sits in a buffer 16 bytes larger, so a size past sizeof()
 * reads the caller's (zeroed) bytes. */
union source_buf {
	struct obs_source_info info;
	unsigned char bytes[sizeof(struct obs_source_info) + 16];
};
union output_buf {
	struct obs_output_info info;
	unsigned char bytes[sizeof(struct obs_output_info) + 16];
};
union encoder_buf {
	struct obs_encoder_info info;
	unsigned char bytes[sizeof(struct obs_encoder_info) + 16];
};
union service_buf {
	struct obs_service_info info;
	unsigned char bytes[sizeof(struct obs_service_info) + 16];
};

/* An input with no output flags: get_name is the only callback it needs. */
static void full_source(union source_buf *b, const char *id)
{
	memset(b, 0, sizeof(*b));
	b->info.id = id;
	b->info.type = OBS_SOURCE_TYPE_INPUT;
	b->info.get_name = get_name;
	b->info.create = source_create;
	b->info.destroy = destroy;
	b->info.get_defaults = get_defaults;
	b->info.type_data = type_data_tag;
	b->info.free_type_data = free_type_data;
	b->info.icon_type = OBS_ICON_TYPE_CAMERA;
}

/* An output with no flags: get_name, create, destroy, start and stop. */
static void full_output(union output_buf *b, const char *id)
{
	memset(b, 0, sizeof(*b));
	b->info.id = id;
	b->info.get_name = get_name;
	b->info.create = output_create;
	b->info.destroy = destroy;
	b->info.start = output_start;
	b->info.stop = output_stop;
	b->info.get_defaults = get_defaults;
	b->info.type_data = type_data_tag;
	b->info.free_type_data = free_type_data;
}

/* A video encoder without texture caps: get_name, create, destroy, encode. */
static void full_encoder(union encoder_buf *b, const char *id)
{
	memset(b, 0, sizeof(*b));
	b->info.id = id;
	b->info.type = OBS_ENCODER_VIDEO;
	b->info.codec = "h264";
	b->info.get_name = get_name;
	b->info.create = encoder_create;
	b->info.destroy = destroy;
	b->info.encode = encode;
	b->info.get_defaults = get_defaults;
	b->info.type_data = type_data_tag;
	b->info.free_type_data = free_type_data;
}

/* A service: get_name, create, destroy and get_protocol. */
static void full_service(union service_buf *b, const char *id)
{
	memset(b, 0, sizeof(*b));
	b->info.id = id;
	b->info.get_name = get_name;
	b->info.create = service_create;
	b->info.destroy = destroy;
	b->info.get_defaults = get_defaults;
	b->info.type_data = type_data_tag;
	b->info.free_type_data = free_type_data;
	b->info.get_protocol = get_protocol;
}

#define END_OF(type, field) (offsetof(type, field) + sizeof(((type *)NULL)->field))

static void begin(void)
{
	logged_num = 0;
	frees = 0;
	defaults_calls = 0;
	capturing = true;
}

static void reg_source(const union source_buf *b, size_t size)
{
	begin();
	obs_register_source_s(&b->info, size);
	capturing = false;
}

static void reg_output(const union output_buf *b, size_t size)
{
	begin();
	obs_register_output_s(&b->info, size);
	capturing = false;
}

static void reg_encoder(const union encoder_buf *b, size_t size)
{
	begin();
	obs_register_encoder_s(&b->info, size);
	capturing = false;
}

static void reg_service(const union service_buf *b, size_t size)
{
	begin();
	obs_register_service_s(&b->info, size);
	capturing = false;
}

/* Whether the registered type's get_defaults was copied: the defaults call
 * it only when it is set. A partly copied pointer would crash here. */
static int source_defaults(const char *id)
{
	defaults_calls = 0;
	obs_data_t *d = obs_get_source_defaults(id);
	assert_non_null(d);
	obs_data_release(d);
	return defaults_calls;
}

static int output_defaults(const char *id)
{
	defaults_calls = 0;
	obs_data_t *d = obs_output_defaults(id);
	assert_non_null(d);
	obs_data_release(d);
	return defaults_calls;
}

static int encoder_defaults(const char *id)
{
	defaults_calls = 0;
	obs_data_t *d = obs_encoder_defaults(id);
	assert_non_null(d);
	obs_data_release(d);
	return defaults_calls;
}

static int service_defaults(const char *id)
{
	defaults_calls = 0;
	obs_data_t *d = obs_service_defaults(id);
	assert_non_null(d);
	obs_data_release(d);
	return defaults_calls;
}

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

/* ---------------------------------------------------------------- sources */

/* get_name counts when it ends exactly at the size, not one byte short. */
static void test_source_callback_at_size(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;
	const size_t end = END_OF(struct obs_source_info, get_name);

	full_source(&b, "mrc_src_name_short");
	reg_source(&b, end - 1);
	expect_logged(1, E, REQUIRED("get_name", "mrc_src_name_short", "obs_register_source"));
	assert_null(obs_source_get_display_name("mrc_src_name_short"));
	assert_int_equal(frees, 0);

	full_source(&b, "mrc_src_name_end");
	reg_source(&b, end);
	expect_logged(0);
	assert_string_equal(obs_source_get_display_name("mrc_src_name_end"), "mrc");
	assert_int_equal(source_defaults("mrc_src_name_end"), 0);
}

/* A size inside get_defaults leaves it zero; at its end, it is copied. */
static void test_source_pointer_cut(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;
	const size_t end = END_OF(struct obs_source_info, get_defaults);

	full_source(&b, "mrc_src_defaults_cut");
	reg_source(&b, end - 1);
	expect_logged(0);
	assert_int_equal(source_defaults("mrc_src_defaults_cut"), 0);

	full_source(&b, "mrc_src_defaults_one");
	reg_source(&b, end - sizeof(void *) + 1);
	expect_logged(0);
	assert_int_equal(source_defaults("mrc_src_defaults_one"), 0);

	full_source(&b, "mrc_src_defaults_end");
	reg_source(&b, end);
	expect_logged(0);
	assert_int_equal(source_defaults("mrc_src_defaults_end"), 1);
}

/* icon_type, a 32-bit field: dropped when the size ends inside it, kept
 * when it lies within the size, also when the size ends inside the pointer
 * after it. */
static void test_source_u32_field(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;
	const size_t end = END_OF(struct obs_source_info, icon_type);
	const size_t next_end = END_OF(struct obs_source_info, media_play_pause);

	full_source(&b, "mrc_src_icon_short");
	reg_source(&b, end - 1);
	expect_logged(0);
	assert_int_equal(obs_source_get_icon_type("mrc_src_icon_short"), OBS_ICON_TYPE_UNKNOWN);

	full_source(&b, "mrc_src_icon_end");
	reg_source(&b, end);
	expect_logged(0);
	assert_int_equal(obs_source_get_icon_type("mrc_src_icon_end"), OBS_ICON_TYPE_CAMERA);

	full_source(&b, "mrc_src_icon_next");
	reg_source(&b, next_end - 1);
	expect_logged(0);
	assert_int_equal(obs_source_get_icon_type("mrc_src_icon_next"), OBS_ICON_TYPE_CAMERA);
	assert_int_equal(source_defaults("mrc_src_icon_next"), 1);
}

/* On an error, the type data is freed only when type_data and
 * free_type_data both lie wholly within the size. */
static void test_source_error_free(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;
	const size_t end = END_OF(struct obs_source_info, free_type_data);

	full_source(&b, "mrc_src_free_cut");
	b.info.get_name = NULL;
	reg_source(&b, end - 1);
	expect_logged(1, E, REQUIRED("get_name", "mrc_src_free_cut", "obs_register_source"));
	assert_int_equal(frees, 0);

	full_source(&b, "mrc_src_free_end");
	b.info.get_name = NULL;
	reg_source(&b, end);
	expect_logged(1, E, REQUIRED("get_name", "mrc_src_free_end", "obs_register_source"));
	assert_int_equal(frees, 1);
}

/* A size of 0 copies nothing, so the source has no get_name: the zero size
 * is never reported as such for a source. Characterized, not endorsed. */
static void test_source_size_zero(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;

	full_source(&b, "mrc_src_zero");
	reg_source(&b, 0);
	expect_logged(1, E, REQUIRED("get_name", "mrc_src_zero", "obs_register_source"));
	assert_int_equal(frees, 0);
	assert_null(obs_source_get_display_name("mrc_src_zero"));
}

/* Past sizeof(): too large, and the type data is freed. */
static void test_source_too_large(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;
	char msg[256];

	full_source(&b, "mrc_src_large");
	reg_source(&b, sizeof(b.info) + 8);
	snprintf(msg, sizeof(msg),
		 "obs_register_source: Tried to register obs_source_info with size %llu which is more than "
		 "libobs currently supports (%llu)",
		 (long long unsigned)(sizeof(b.info) + 8), (long long unsigned)sizeof(b.info));
	expect_logged(1, W, msg);
	assert_int_equal(frees, 1);
	assert_null(obs_source_get_display_name("mrc_src_large"));
}

/* A transition's get_width and get_height are each warned about, and the
 * transition still registers. */
static void test_source_transition_sizes(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union source_buf b;

	full_source(&b, "mrc_tr_both");
	b.info.type = OBS_SOURCE_TYPE_TRANSITION;
	b.info.audio_render = audio_render;
	b.info.get_width = get_size;
	b.info.get_height = get_size;
	reg_source(&b, sizeof(b.info));
	expect_logged(2, W, "obs_register_source: get_width ignored registering transition 'mrc_tr_both'", W,
		      "obs_register_source: get_height ignored registering transition 'mrc_tr_both'");
	assert_string_equal(obs_source_get_display_name("mrc_tr_both"), "mrc");

	full_source(&b, "mrc_tr_height");
	b.info.type = OBS_SOURCE_TYPE_TRANSITION;
	b.info.audio_render = audio_render;
	b.info.get_height = get_size;
	reg_source(&b, sizeof(b.info));
	expect_logged(1, W, "obs_register_source: get_height ignored registering transition 'mrc_tr_height'");

	/* An input with both registers without a warning. */
	full_source(&b, "mrc_in_sizes");
	b.info.get_width = get_size;
	b.info.get_height = get_size;
	reg_source(&b, sizeof(b.info));
	expect_logged(0);
}

/* ---------------------------------------------------------------- outputs */

static void test_output_sizes(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union output_buf b;
	const size_t stop_end = END_OF(struct obs_output_info, stop);
	const size_t defaults_end = END_OF(struct obs_output_info, get_defaults);
	const size_t free_end = END_OF(struct obs_output_info, free_type_data);
	char msg[256];

	/* stop counts when it ends exactly at the size. */
	full_output(&b, "mrc_out_stop_short");
	reg_output(&b, stop_end - 1);
	expect_logged(1, E, REQUIRED("stop", "mrc_out_stop_short", "obs_register_output"));
	assert_null(obs_output_get_display_name("mrc_out_stop_short"));

	full_output(&b, "mrc_out_stop_end");
	reg_output(&b, stop_end);
	expect_logged(0);
	assert_string_equal(obs_output_get_display_name("mrc_out_stop_end"), "mrc");
	assert_int_equal(output_defaults("mrc_out_stop_end"), 0);

	/* A size inside get_defaults leaves it zero. */
	full_output(&b, "mrc_out_defaults_cut");
	reg_output(&b, defaults_end - 1);
	expect_logged(0);
	assert_int_equal(output_defaults("mrc_out_defaults_cut"), 0);

	full_output(&b, "mrc_out_defaults_end");
	reg_output(&b, defaults_end);
	expect_logged(0);
	assert_int_equal(output_defaults("mrc_out_defaults_end"), 1);

	/* The type data on an error. */
	full_output(&b, "mrc_out_free_cut");
	b.info.get_name = NULL;
	reg_output(&b, free_end - 1);
	expect_logged(1, E, REQUIRED("get_name", "mrc_out_free_cut", "obs_register_output"));
	assert_int_equal(frees, 0);

	full_output(&b, "mrc_out_free_end");
	b.info.get_name = NULL;
	reg_output(&b, free_end);
	expect_logged(1, E, REQUIRED("get_name", "mrc_out_free_end", "obs_register_output"));
	assert_int_equal(frees, 1);

	/* A size of 0: the callback check fails first, so "outside of
	 * obs_module_load" is never logged. Characterized, not endorsed. */
	full_output(&b, "mrc_out_zero");
	reg_output(&b, 0);
	expect_logged(1, E, REQUIRED("get_name", "mrc_out_zero", "obs_register_output"));
	assert_int_equal(frees, 0);

	/* Past sizeof(). */
	full_output(&b, "mrc_out_large");
	reg_output(&b, sizeof(b.info) + 8);
	snprintf(msg, sizeof(msg),
		 "Tried to register obs_output_info with size %llu which is more than libobs currently supports (%llu)",
		 (long long unsigned)(sizeof(b.info) + 8), (long long unsigned)sizeof(b.info));
	expect_logged(1, E, msg);
	assert_int_equal(frees, 1);
	assert_null(obs_output_get_display_name("mrc_out_large"));
}

/* --------------------------------------------------------------- encoders */

static void test_encoder_sizes(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union encoder_buf b;
	const size_t encode_end = END_OF(struct obs_encoder_info, encode);
	const size_t defaults_end = END_OF(struct obs_encoder_info, get_defaults);
	const size_t free_end = END_OF(struct obs_encoder_info, free_type_data);
	char msg[256];

	full_encoder(&b, "mrc_enc_encode_short");
	reg_encoder(&b, encode_end - 1);
	expect_logged(1, E, REQUIRED("encode", "mrc_enc_encode_short", "obs_register_encoder"));
	assert_null(obs_encoder_get_display_name("mrc_enc_encode_short"));

	full_encoder(&b, "mrc_enc_encode_end");
	reg_encoder(&b, encode_end);
	expect_logged(0);
	assert_string_equal(obs_encoder_get_display_name("mrc_enc_encode_end"), "mrc");
	assert_int_equal(encoder_defaults("mrc_enc_encode_end"), 0);

	full_encoder(&b, "mrc_enc_defaults_cut");
	reg_encoder(&b, defaults_end - 1);
	expect_logged(0);
	assert_int_equal(encoder_defaults("mrc_enc_defaults_cut"), 0);

	full_encoder(&b, "mrc_enc_defaults_end");
	reg_encoder(&b, defaults_end);
	expect_logged(0);
	assert_int_equal(encoder_defaults("mrc_enc_defaults_end"), 1);

	full_encoder(&b, "mrc_enc_free_cut");
	b.info.get_name = NULL;
	reg_encoder(&b, free_end - 1);
	expect_logged(1, E, REQUIRED("get_name", "mrc_enc_free_cut", "obs_register_encoder"));
	assert_int_equal(frees, 0);

	full_encoder(&b, "mrc_enc_free_end");
	b.info.get_name = NULL;
	reg_encoder(&b, free_end);
	expect_logged(1, E, REQUIRED("get_name", "mrc_enc_free_end", "obs_register_encoder"));
	assert_int_equal(frees, 1);

	/* Characterized, not endorsed: the callback check fails first. */
	full_encoder(&b, "mrc_enc_zero");
	reg_encoder(&b, 0);
	expect_logged(1, E, REQUIRED("get_name", "mrc_enc_zero", "obs_register_encoder"));
	assert_int_equal(frees, 0);

	full_encoder(&b, "mrc_enc_large");
	reg_encoder(&b, sizeof(b.info) + 8);
	snprintf(
		msg, sizeof(msg),
		"Tried to register obs_encoder_info with size %llu which is more than libobs currently supports (%llu)",
		(long long unsigned)(sizeof(b.info) + 8), (long long unsigned)sizeof(b.info));
	expect_logged(1, E, msg);
	assert_int_equal(frees, 1);
	assert_null(obs_encoder_get_display_name("mrc_enc_large"));
}

/* --------------------------------------------------------------- services */

static void test_service_sizes(void **state)
{
	(void)state;
	REQUIRE_OBS();
	union service_buf b;
	const size_t protocol_end = END_OF(struct obs_service_info, get_protocol);
	const size_t defaults_end = END_OF(struct obs_service_info, get_defaults);
	const size_t free_end = END_OF(struct obs_service_info, free_type_data);
	char msg[256];

	full_service(&b, "mrc_svc_protocol_short");
	reg_service(&b, protocol_end - 1);
	expect_logged(1, E, REQUIRED("get_protocol", "mrc_svc_protocol_short", "obs_register_service"));
	assert_null(obs_service_get_display_name("mrc_svc_protocol_short"));
	assert_int_equal(frees, 1);

	full_service(&b, "mrc_svc_protocol_end");
	reg_service(&b, protocol_end);
	expect_logged(0);
	assert_string_equal(obs_service_get_display_name("mrc_svc_protocol_end"), "mrc");
	assert_int_equal(service_defaults("mrc_svc_protocol_end"), 1);

	/* get_protocol lies after get_defaults, so a service is registered only
	 * from a size past it: a cut inside get_defaults never registers. */
	full_service(&b, "mrc_svc_defaults_cut");
	reg_service(&b, defaults_end - 1);
	expect_logged(1, E, REQUIRED("get_protocol", "mrc_svc_defaults_cut", "obs_register_service"));
	assert_int_equal(frees, 0);

	full_service(&b, "mrc_svc_free_cut");
	b.info.get_name = NULL;
	reg_service(&b, free_end - 1);
	expect_logged(1, E, REQUIRED("get_name", "mrc_svc_free_cut", "obs_register_service"));
	assert_int_equal(frees, 0);

	full_service(&b, "mrc_svc_free_end");
	b.info.get_name = NULL;
	reg_service(&b, free_end);
	expect_logged(1, E, REQUIRED("get_name", "mrc_svc_free_end", "obs_register_service"));
	assert_int_equal(frees, 1);

	/* Characterized, not endorsed: the callback check fails first. */
	full_service(&b, "mrc_svc_zero");
	reg_service(&b, 0);
	expect_logged(1, E, REQUIRED("get_name", "mrc_svc_zero", "obs_register_service"));
	assert_int_equal(frees, 0);

	full_service(&b, "mrc_svc_large");
	reg_service(&b, sizeof(b.info) + 8);
	snprintf(
		msg, sizeof(msg),
		"Tried to register obs_service_info with size %llu which is more than libobs currently supports (%llu)",
		(long long unsigned)(sizeof(b.info) + 8), (long long unsigned)sizeof(b.info));
	expect_logged(1, E, msg);
	assert_int_equal(frees, 1);
	assert_null(obs_service_get_display_name("mrc_svc_large"));
}

#define CASE(fn) cmocka_unit_test(fn)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_whole_size_pointers_only),
		CASE(test_whole_size_source_head),
		CASE(test_whole_size_padded_field),
		CASE(test_whole_size_32bit),
		CASE(test_whole_size_order),
		CASE(test_within),
		CASE(test_gate),
		CASE(test_ignores_size),
		CASE(test_source_callback_at_size),
		CASE(test_source_pointer_cut),
		CASE(test_source_u32_field),
		CASE(test_source_error_free),
		CASE(test_source_size_zero),
		CASE(test_source_too_large),
		CASE(test_source_transition_sizes),
		CASE(test_output_sizes),
		CASE(test_encoder_sizes),
		CASE(test_service_sizes),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
