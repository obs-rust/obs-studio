/* Regression test for obs-rust/obs-studio#2932: obs_encoder_set_group()
 * took the encoder out of its old group, and dropped the old group's
 * reference on it, before it checked whether the new group has started
 * encoders. A move into a started group then returned false with the
 * encoder in no group: the old group no longer listed it.
 *
 * A group counts as started while one of its encoders runs. The test marks
 * a group started by setting num_encoders_started under its mutex, as
 * add_connection() does, so it needs no video, output or graphics module.
 * The other cases pin what already worked: a move between two idle groups,
 * a move out of a started group (refused, as before), and the same group
 * set again (still one membership and one reference).
 *
 * libobs needs an X display on Linux, so the cases skip without one (run
 * under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs-internal.h>

#define VENC_ID "test_group_move_encoder"

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "group move encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}

static void enc_destroy(void *data)
{
	bfree(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static bool obs_up;

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_encoder_info info = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	obs_register_encoder(&info);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

static void set_started(obs_encoder_group_t *group, uint32_t started)
{
	pthread_mutex_lock(&group->mutex);
	group->num_encoders_started = started;
	pthread_mutex_unlock(&group->mutex);
}

static size_t group_size(obs_encoder_group_t *group)
{
	pthread_mutex_lock(&group->mutex);
	size_t num = group->encoders.num;
	pthread_mutex_unlock(&group->mutex);
	return num;
}

static bool group_lists(obs_encoder_group_t *group, obs_encoder_t *encoder)
{
	bool found = false;
	pthread_mutex_lock(&group->mutex);
	for (size_t i = 0; i < group->encoders.num; i++) {
		if (group->encoders.array[i] == encoder)
			found = true;
	}
	pthread_mutex_unlock(&group->mutex);
	return found;
}

/* Releases the test's own reference and reports whether a group's
 * reference still keeps the encoder alive. */
static bool alive_after_release(obs_encoder_t *encoder, obs_weak_encoder_t *weak)
{
	obs_encoder_release(encoder);
	obs_encoder_t *alive = obs_weak_encoder_get_encoder(weak);
	obs_encoder_release(alive);
	return alive != NULL;
}

static void test_move_into_started_group_keeps_old_group(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "into started", NULL, NULL);
	assert_non_null(enc);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(enc);
	obs_encoder_group_t *old_group = obs_encoder_group_create();
	obs_encoder_group_t *started = obs_encoder_group_create();
	assert_non_null(old_group);
	assert_non_null(started);

	assert_true(obs_encoder_set_group(enc, old_group));
	set_started(started, 1);

	assert_false(obs_encoder_set_group(enc, started));
	assert_ptr_equal(enc->encoder_group, old_group);
	assert_true(group_lists(old_group, enc));
	assert_int_equal(group_size(started), 0);

	/* The old group still holds its reference. */
	assert_true(alive_after_release(enc, weak));

	set_started(started, 0);
	obs_encoder_group_destroy(started);
	obs_encoder_group_destroy(old_group);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
}

static void test_move_out_of_started_group_is_refused(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "out of started", NULL, NULL);
	assert_non_null(enc);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(enc);
	obs_encoder_group_t *started = obs_encoder_group_create();
	obs_encoder_group_t *idle = obs_encoder_group_create();

	assert_true(obs_encoder_set_group(enc, started));
	set_started(started, 1);

	assert_false(obs_encoder_set_group(enc, idle));
	assert_false(obs_encoder_set_group(enc, NULL));
	assert_ptr_equal(enc->encoder_group, started);
	assert_true(group_lists(started, enc));
	assert_int_equal(group_size(idle), 0);

	assert_true(alive_after_release(enc, weak));

	set_started(started, 0);
	obs_encoder_group_destroy(idle);
	obs_encoder_group_destroy(started);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
}

static void test_move_between_idle_groups(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "between idle", NULL, NULL);
	assert_non_null(enc);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(enc);
	obs_encoder_group_t *first = obs_encoder_group_create();
	obs_encoder_group_t *second = obs_encoder_group_create();

	assert_true(obs_encoder_set_group(enc, first));
	assert_true(obs_encoder_set_group(enc, second));
	assert_ptr_equal(enc->encoder_group, second);
	assert_int_equal(group_size(first), 0);
	assert_true(group_lists(second, enc));

	assert_true(alive_after_release(enc, weak));

	/* The first group dropped its reference: only the second's is left. */
	obs_encoder_group_destroy(first);
	assert_true(alive_after_release(obs_weak_encoder_get_encoder(weak), weak));
	obs_encoder_group_destroy(second);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
}

static void test_same_group_again_keeps_one_membership(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "same group", NULL, NULL);
	assert_non_null(enc);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(enc);
	obs_encoder_group_t *group = obs_encoder_group_create();

	assert_true(obs_encoder_set_group(enc, group));
	assert_true(obs_encoder_set_group(enc, group));
	assert_ptr_equal(enc->encoder_group, group);
	assert_int_equal(group_size(group), 1);

	assert_true(alive_after_release(enc, weak));

	/* One reference for one membership: destroying the group frees it. */
	obs_encoder_group_destroy(group);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_move_into_started_group_keeps_old_group),
		cmocka_unit_test(test_move_out_of_started_group_is_refused),
		cmocka_unit_test(test_move_between_idle_groups),
		cmocka_unit_test(test_same_group_again_keeps_one_membership),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
