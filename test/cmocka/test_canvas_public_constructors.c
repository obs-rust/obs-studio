/* Characterize obs_canvas_create() and obs_canvas_create_private() before the
 * Rust swap (Refs #5131). test_canvas_rules_callers::constructor_flags and
 * test_canvas_flags pin the flag mask; this adds what the two wrappers forward
 * to the internal creator: the name (NULL and "" included), a NULL uuid (so a
 * fresh one is generated), the video info pointer, public versus private
 * registration and signals, and the single reference the caller owns.
 * No graphics needed: an invalid video info is copied without opening a mix. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "obs-internal.h"

#if !__has_include("obs-canvas-public-constructors.c.h")
#error "canvas public constructor extraction seam is missing"
#endif

#define REMOVED_BIT (1u << 31)

static void assert_same_info(const struct obs_video_info *a, const struct obs_video_info *b)
{
	assert_ptr_equal(a->graphics_module, b->graphics_module);
	assert_int_equal(a->fps_num, b->fps_num);
	assert_int_equal(a->fps_den, b->fps_den);
	assert_int_equal(a->base_width, b->base_width);
	assert_int_equal(a->base_height, b->base_height);
	assert_int_equal(a->output_width, b->output_width);
	assert_int_equal(a->output_height, b->output_height);
	assert_int_equal(a->output_format, b->output_format);
	assert_int_equal(a->adapter, b->adapter);
	assert_int_equal(a->gpu_conversion, b->gpu_conversion);
	assert_int_equal(a->colorspace, b->colorspace);
	assert_int_equal(a->range, b->range);
	assert_int_equal(a->scale_type, b->scale_type);
}

struct create_log {
	obs_canvas_t *seen[4];
	size_t count;
};

static void create_cb(void *param, calldata_t *data)
{
	struct create_log *log = param;
	assert_true(log->count < 4);
	log->seen[log->count++] = calldata_ptr(data, "canvas");
}

static void connect_log(struct create_log *log)
{
	signal_handler_connect(obs_get_signal_handler(), "canvas_create", create_cb, log);
}

static void disconnect_log(struct create_log *log)
{
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_create", create_cb, log);
}

static long strong_refs(obs_canvas_t *canvas)
{
	return os_atomic_load_long(&canvas->context.control->ref.refs);
}

static bool in_name_table(obs_canvas_t *canvas)
{
	obs_canvas_t *found = obs_get_canvas_by_name(canvas->context.name);
	obs_canvas_release(found);
	return found == canvas;
}

static bool in_uuid_table(obs_canvas_t *canvas)
{
	obs_canvas_t *found = obs_get_canvas_by_uuid(canvas->context.uuid);
	obs_canvas_release(found);
	return found == canvas;
}

static void assert_fresh_uuid(obs_canvas_t *canvas)
{
	assert_non_null(canvas->context.uuid);
	assert_int_equal(strlen(canvas->context.uuid), UUID_STR_LENGTH);
}

static void public_create_forwards(void **unused)
{
	(void)unused;
	struct create_log log = {0};
	connect_log(&log);

	obs_canvas_t *canvas = obs_canvas_create("ctor public", NULL, PROGRAM | MAIN | REMOVED_BIT | (1u << 5));
	assert_non_null(canvas);

	assert_string_equal(canvas->context.name, "ctor public");
	assert_false(canvas->context.private);
	assert_int_equal(canvas->flags, PROGRAM);
	assert_null(canvas->mix);
	assert_fresh_uuid(canvas);
	/* Public: in both tables, one global canvas_create with the new canvas. */
	assert_true(in_name_table(canvas));
	assert_true(in_uuid_table(canvas));
	assert_int_equal(log.count, 1);
	assert_ptr_equal(log.seen[0], canvas);
	/* The caller owns the only reference. */
	assert_int_equal(strong_refs(canvas), 0);

	disconnect_log(&log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void private_create_forwards(void **unused)
{
	(void)unused;
	struct create_log log = {0};
	connect_log(&log);

	obs_canvas_t *canvas = obs_canvas_create_private("ctor private", NULL, 0xffffffffu);
	assert_non_null(canvas);

	assert_string_equal(canvas->context.name, "ctor private");
	assert_true(canvas->context.private);
	assert_int_equal(canvas->flags, PROGRAM | EPHEMERAL);
	assert_fresh_uuid(canvas);
	/* Private: uuid table only, and no global signal. */
	assert_false(in_name_table(canvas));
	assert_true(in_uuid_table(canvas));
	assert_int_equal(log.count, 0);
	assert_int_equal(strong_refs(canvas), 0);

	disconnect_log(&log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void uuids_are_generated_per_call(void **unused)
{
	(void)unused;
	obs_canvas_t *a = obs_canvas_create("ctor uuid", NULL, 0);
	obs_canvas_t *b = obs_canvas_create_private("ctor uuid", NULL, 0);
	assert_non_null(a);
	assert_non_null(b);
	assert_fresh_uuid(a);
	assert_fresh_uuid(b);
	assert_string_not_equal(a->context.uuid, b->context.uuid);
	obs_canvas_release(a);
	obs_canvas_release(b);
	obs_wait_for_destroy_queue();
}

static void blank_names(void **unused)
{
	(void)unused;
	/* Public NULL and "" get a generated name; private "" does too, but a
	 * private NULL name stays NULL. */
	obs_canvas_t *pub_null = obs_canvas_create(NULL, NULL, 0);
	obs_canvas_t *pub_empty = obs_canvas_create("", NULL, 0);
	obs_canvas_t *priv_null = obs_canvas_create_private(NULL, NULL, 0);
	obs_canvas_t *priv_empty = obs_canvas_create_private("", NULL, 0);
	assert_non_null(pub_null);
	assert_non_null(pub_empty);
	assert_non_null(priv_null);
	assert_non_null(priv_empty);

	assert_non_null(pub_null->context.name);
	assert_int_equal(strncmp(pub_null->context.name, "__unnamed", 9), 0);
	assert_non_null(pub_empty->context.name);
	assert_int_equal(strncmp(pub_empty->context.name, "__unnamed", 9), 0);
	assert_string_not_equal(pub_null->context.name, pub_empty->context.name);
	assert_null(priv_null->context.name);
	assert_non_null(priv_empty->context.name);
	assert_int_equal(strncmp(priv_empty->context.name, "__unnamed", 9), 0);
	assert_true(in_name_table(pub_null));
	assert_true(in_name_table(pub_empty));

	obs_canvas_release(pub_null);
	obs_canvas_release(pub_empty);
	obs_canvas_release(priv_null);
	obs_canvas_release(priv_empty);
	obs_wait_for_destroy_queue();
}

static void video_info_is_forwarded(void **unused)
{
	(void)unused;
	/* An output width of 0 is refused before any graphics work, so the copy
	 * of the forwarded info is observable without a display. */
	struct obs_video_info ovi = {
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 32,
		.base_height = 48,
		.output_width = 0,
		.output_height = 48,
		.output_format = VIDEO_FORMAT_NV12,
	};
	obs_canvas_t *pub = obs_canvas_create("ctor ovi public", &ovi, PROGRAM);
	obs_canvas_t *priv = obs_canvas_create_private("ctor ovi private", &ovi, PROGRAM);
	assert_non_null(pub);
	assert_non_null(priv);
	assert_same_info(&pub->ovi, &ovi);
	assert_same_info(&priv->ovi, &ovi);
	assert_null(pub->mix);
	assert_null(priv->mix);
	obs_canvas_release(pub);
	obs_canvas_release(priv);
	obs_wait_for_destroy_queue();
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(public_create_forwards),       cmocka_unit_test(private_create_forwards),
		cmocka_unit_test(uuids_are_generated_per_call), cmocka_unit_test(blank_names),
		cmocka_unit_test(video_info_is_forwarded),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return result;
}
