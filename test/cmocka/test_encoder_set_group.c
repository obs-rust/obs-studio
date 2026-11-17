/* Actual-library characterization of obs_encoder_set_group() before its
 * body moves out of obs-encoder.c.h (obs-rust/obs-studio#4645). It pins
 * every return path with its exact log, the membership and reference each
 * path leaves, and the check order: an active encoder is refused first, the
 * same-group no-op comes before the started check, and the destination is
 * checked before the encoder leaves its old group. test_encoder_group_move,
 * test_encoder_group_null and test_encoder_group_reconfig_walk keep their
 * own cases.
 *
 * A group counts as started while num_encoders_started is non-zero; the test
 * sets it under the group mutex, as add_connection() does. An encoder counts
 * as active while its active flag is set. Historical log spelling is
 * characterized, not endorsed. libobs needs an X display on Linux, so the
 * cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <obs-internal.h>

#define VENC_ID "test_set_group_encoder"

static unsigned log_count;
static int log_level;
static char log_message[256];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "set group encoder";
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

static int setup(void **state)
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

static int teardown(void **state)
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

static obs_encoder_t *group_at(obs_encoder_group_t *group, size_t i)
{
	pthread_mutex_lock(&group->mutex);
	obs_encoder_t *encoder = i < group->encoders.num ? group->encoders.array[i] : NULL;
	pthread_mutex_unlock(&group->mutex);
	return encoder;
}

static long refs_of(obs_encoder_t *encoder)
{
	return encoder->context.control->ref.refs;
}

/* Calls obs_encoder_set_group with the log captured and checks the result,
 * the number of log lines and, for one line, its level and text. */
static void expect_set(obs_encoder_t *encoder, obs_encoder_group_t *group, bool result, const char *message)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_message[0] = '\0';
	bool got = obs_encoder_set_group(encoder, group);
	unsigned count = log_count;
	int level = log_level;
	base_set_log_handler(old_handler, old_param);

	assert_int_equal(got, result);
	if (!message) {
		assert_int_equal(count, 0);
		return;
	}
	assert_int_equal(count, 1);
	assert_int_equal(level, encoder ? LOG_ERROR : LOG_DEBUG);
	assert_string_equal(log_message, message);
}

static void null_encoder(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_group_t *group = obs_encoder_group_create();
	assert_non_null(group);
	expect_set(NULL, NULL, false, "obs_encoder_set_group: Null 'encoder' parameter");
	expect_set(NULL, group, false, "obs_encoder_set_group: Null 'encoder' parameter");
	assert_int_equal(group_size(group), 0);
	obs_encoder_group_destroy(group);
}

static void active_encoder_is_refused_first(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "active", NULL, NULL);
	assert_non_null(enc);
	obs_encoder_group_t *home = obs_encoder_group_create();
	obs_encoder_group_t *other = obs_encoder_group_create();
	assert_true(obs_encoder_set_group(enc, home));
	long refs = refs_of(enc);

	os_atomic_set_bool(&enc->active, true);
	/* Refused before the same-group, started and NULL-group paths. */
	expect_set(enc, home, false, "obs_encoder_set_group: encoder 'active' is already active");
	expect_set(enc, other, false, "obs_encoder_set_group: encoder 'active' is already active");
	expect_set(enc, NULL, false, "obs_encoder_set_group: encoder 'active' is already active");
	set_started(other, 1);
	expect_set(enc, other, false, "obs_encoder_set_group: encoder 'active' is already active");
	set_started(other, 0);
	os_atomic_set_bool(&enc->active, false);

	assert_ptr_equal(enc->encoder_group, home);
	assert_int_equal(group_size(home), 1);
	assert_ptr_equal(group_at(home, 0), enc);
	assert_int_equal(group_size(other), 0);
	assert_int_equal(refs_of(enc), refs);

	obs_encoder_group_destroy(other);
	obs_encoder_group_destroy(home);
	obs_encoder_release(enc);
}

static void null_group_without_a_group_is_a_noop(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "loose", NULL, NULL);
	assert_non_null(enc);
	long refs = refs_of(enc);
	expect_set(enc, NULL, true, NULL);
	expect_set(enc, NULL, true, NULL);
	assert_null(enc->encoder_group);
	assert_int_equal(refs_of(enc), refs);
	obs_encoder_release(enc);
}

static void join_takes_one_reference(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "joiner", NULL, NULL);
	assert_non_null(enc);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(enc);
	obs_encoder_group_t *group = obs_encoder_group_create();
	long before = refs_of(enc);

	expect_set(enc, group, true, NULL);
	assert_ptr_equal(enc->encoder_group, group);
	assert_int_equal(group_size(group), 1);
	assert_ptr_equal(group_at(group, 0), enc);
	assert_int_equal(refs_of(enc), before + 1);

	/* The same group again: no second membership or reference. */
	expect_set(enc, group, true, NULL);
	assert_int_equal(group_size(group), 1);
	assert_int_equal(refs_of(enc), before + 1);

	/* Leaving drops the group's reference. */
	expect_set(enc, NULL, true, NULL);
	assert_null(enc->encoder_group);
	assert_int_equal(group_size(group), 0);
	assert_int_equal(refs_of(enc), before);

	/* The group's reference outlives the caller's, and leaving the group
	 * then destroys the encoder. */
	expect_set(enc, group, true, NULL);
	obs_encoder_release(enc);
	obs_encoder_t *back = obs_weak_encoder_get_encoder(weak);
	assert_ptr_equal(back, enc);
	assert_ptr_equal(group_at(group, 0), back);
	expect_set(back, NULL, true, NULL);
	obs_encoder_release(back);
	assert_null(obs_weak_encoder_get_encoder(weak));

	obs_weak_encoder_release(weak);
	obs_encoder_group_destroy(group);
}

static void same_group_wins_over_started(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "same", NULL, NULL);
	assert_non_null(enc);
	obs_encoder_group_t *group = obs_encoder_group_create();
	assert_true(obs_encoder_set_group(enc, group));
	long refs = refs_of(enc);

	set_started(group, 1);
	expect_set(enc, group, true, NULL);
	assert_int_equal(group_size(group), 1);
	assert_int_equal(refs_of(enc), refs);
	set_started(group, 0);

	obs_encoder_group_destroy(group);
	obs_encoder_release(enc);
}

static void started_destination_is_refused_before_leaving(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "mover", NULL, NULL);
	assert_non_null(enc);
	obs_encoder_group_t *home = obs_encoder_group_create();
	obs_encoder_group_t *started = obs_encoder_group_create();
	long refs = refs_of(enc);

	/* From no group: one log, nothing joined. */
	set_started(started, 2);
	expect_set(enc, started, false, "obs_encoder_set_group: specified group has started encoders");
	assert_null(enc->encoder_group);
	assert_int_equal(group_size(started), 0);
	assert_int_equal(refs_of(enc), refs);

	/* From an idle group: one log (the preflight), still in the old one. */
	assert_true(obs_encoder_set_group(enc, home));
	expect_set(enc, started, false, "obs_encoder_set_group: specified group has started encoders");
	assert_ptr_equal(enc->encoder_group, home);
	assert_ptr_equal(group_at(home, 0), enc);
	assert_int_equal(group_size(started), 0);
	assert_int_equal(refs_of(enc), refs + 1);

	/* From a started group into a started group: the destination is
	 * checked first, so its message is the one logged. */
	set_started(home, 1);
	expect_set(enc, started, false, "obs_encoder_set_group: specified group has started encoders");
	assert_ptr_equal(enc->encoder_group, home);
	set_started(home, 0);
	set_started(started, 0);

	obs_encoder_group_destroy(started);
	obs_encoder_group_destroy(home);
	obs_encoder_release(enc);
}

static void started_old_group_is_refused(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "stuck", NULL, NULL);
	assert_non_null(enc);
	obs_encoder_group_t *home = obs_encoder_group_create();
	obs_encoder_group_t *idle = obs_encoder_group_create();
	assert_true(obs_encoder_set_group(enc, home));
	long refs = refs_of(enc);

	set_started(home, 1);
	expect_set(enc, NULL, false, "obs_encoder_set_group: encoder 'stuck' existing group has started encoders");
	expect_set(enc, idle, false, "obs_encoder_set_group: encoder 'stuck' existing group has started encoders");
	assert_ptr_equal(enc->encoder_group, home);
	assert_ptr_equal(group_at(home, 0), enc);
	assert_int_equal(group_size(idle), 0);
	assert_int_equal(refs_of(enc), refs);
	set_started(home, 0);

	/* Idle again: the move goes through and keeps one reference. */
	expect_set(enc, idle, true, NULL);
	assert_ptr_equal(enc->encoder_group, idle);
	assert_int_equal(group_size(home), 0);
	assert_ptr_equal(group_at(idle, 0), enc);
	assert_int_equal(refs_of(enc), refs);

	obs_encoder_group_destroy(idle);
	obs_encoder_group_destroy(home);
	obs_encoder_release(enc);
}

static void leaving_keeps_the_other_members(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_encoder_t *a = obs_video_encoder_create(VENC_ID, "a", NULL, NULL);
	obs_encoder_t *b = obs_video_encoder_create(VENC_ID, "b", NULL, NULL);
	obs_encoder_t *c = obs_video_encoder_create(VENC_ID, "c", NULL, NULL);
	assert_non_null(a);
	assert_non_null(b);
	assert_non_null(c);
	obs_encoder_group_t *group = obs_encoder_group_create();
	assert_true(obs_encoder_set_group(a, group));
	assert_true(obs_encoder_set_group(b, group));
	assert_true(obs_encoder_set_group(c, group));

	expect_set(b, NULL, true, NULL);
	assert_int_equal(group_size(group), 2);
	assert_ptr_equal(group_at(group, 0), a);
	assert_ptr_equal(group_at(group, 1), c);
	assert_ptr_equal(a->encoder_group, group);
	assert_null(b->encoder_group);
	assert_ptr_equal(c->encoder_group, group);

	/* Rejoining appends at the end. */
	expect_set(b, group, true, NULL);
	assert_int_equal(group_size(group), 3);
	assert_ptr_equal(group_at(group, 2), b);

	obs_encoder_group_destroy(group);
	obs_encoder_release(c);
	obs_encoder_release(b);
	obs_encoder_release(a);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_encoder),
		cmocka_unit_test(active_encoder_is_refused_first),
		cmocka_unit_test(null_group_without_a_group_is_a_noop),
		cmocka_unit_test(join_takes_one_reference),
		cmocka_unit_test(same_group_wins_over_started),
		cmocka_unit_test(started_destination_is_refused_before_leaving),
		cmocka_unit_test(started_old_group_is_refused),
		cmocka_unit_test(leaving_keeps_the_other_members),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
