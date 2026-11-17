/* Actual-library coverage of the encoder-group Rust symbol swap. The source
 * characterization remains unchanged; this executable links only libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

static const char *group_api_name(void *data)
{
	(void)data;
	return "group lifecycle API";
}

static void *group_api_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}

static void group_api_destroy(void *data)
{
	bfree(data);
}

static bool group_api_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static void creation_and_empty_deferred_destroy(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	obs_encoder_group_destroy(NULL);
	obs_encoder_group_t *group = obs_encoder_group_create();
	assert_non_null(group);
	assert_false(group->destroy_on_stop);
	assert_null(group->encoders.array);
	assert_int_equal(group->encoders.num, 0);
	assert_int_equal(group->encoders.capacity, 0);
	assert_int_equal(group->num_encoders_started, 0);
	assert_int_equal(group->start_timestamp, 0);
	assert_int_equal(group->frame_rate_divisors_lcm, 0);
	assert_int_equal(group->reconfigure_request, 0);
	assert_int_equal(group->next_pts, 0);
	assert_int_equal(group->encoders_updated_next_pts, 0);
	assert_int_equal(group->encoders_reconfigured, 0);
	assert_false(group->reconfigure_again);
	assert_int_equal(pthread_mutex_trylock(&group->mutex), 0);
	group->num_encoders_started = 2;
	pthread_mutex_unlock(&group->mutex);
	for (size_t i = 0; i < 2; i++) {
		obs_encoder_group_destroy(group);
		assert_true(group->destroy_on_stop);
		assert_int_equal(group->num_encoders_started, 2);
		assert_int_equal(pthread_mutex_trylock(&group->mutex), 0);
		pthread_mutex_unlock(&group->mutex);
	}
	pthread_mutex_lock(&group->mutex);
	group->num_encoders_started = 0;
	pthread_mutex_unlock(&group->mutex);
	obs_encoder_group_destroy(group);
	assert_int_equal(bnum_allocs(), baseline);
}

static void strong_references_and_back_pointers(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_encoder_info info = {
		.id = "group_lifecycle_api",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = group_api_name,
		.create = group_api_create,
		.destroy = group_api_destroy,
		.encode = group_api_encode,
	};
	obs_register_encoder(&info);
	for (size_t count = 0; count <= 3; count++) {
		obs_encoder_t *encoders[3] = {0};
		obs_weak_encoder_t *weak[3] = {0};
		obs_encoder_group_t *group = obs_encoder_group_create();
		assert_non_null(group);
		for (size_t i = 0; i < count; i++) {
			encoders[i] = obs_video_encoder_create(info.id, "group lifecycle", NULL, NULL);
			assert_non_null(encoders[i]);
			weak[i] = obs_encoder_get_weak_encoder(encoders[i]);
			assert_true(obs_encoder_set_group(encoders[i], group));
		}
		pthread_mutex_lock(&group->mutex);
		group->num_encoders_started = UINT32_MAX;
		pthread_mutex_unlock(&group->mutex);
		for (size_t repeat = 0; repeat < 2; repeat++) {
			obs_encoder_group_destroy(group);
			assert_true(group->destroy_on_stop);
			assert_int_equal(group->encoders.num, count);
			for (size_t i = 0; i < count; i++)
				assert_ptr_equal(encoders[i]->encoder_group, group);
		}
		/* Odd cases leave the group as the sole owner. Even cases retain a
		 * caller reference so the cleared back-pointer can be inspected. */
		if (count % 2)
			for (size_t i = 0; i < count; i++)
				obs_encoder_release(encoders[i]);
		pthread_mutex_lock(&group->mutex);
		group->num_encoders_started = 0;
		pthread_mutex_unlock(&group->mutex);
		obs_encoder_group_destroy(group);
		for (size_t i = 0; i < count; i++) {
			if (!(count % 2)) {
				assert_null(encoders[i]->encoder_group);
				obs_encoder_release(encoders[i]);
			}
			assert_null(obs_weak_encoder_get_encoder(weak[i]));
			obs_weak_encoder_release(weak[i]);
		}
	}
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(creation_and_empty_deferred_destroy),
					   cmocka_unit_test(strong_references_and_back_pointers)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
