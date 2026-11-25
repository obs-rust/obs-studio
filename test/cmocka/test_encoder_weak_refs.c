/* Characterization of the actual libobs encoder weak-reference API. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void null_handles(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	obs_weak_encoder_addref(NULL);
	obs_weak_encoder_release(NULL);
	assert_null(obs_encoder_get_weak_encoder(NULL));
	struct obs_encoder encoder = {0};
	assert_null(obs_encoder_get_weak_encoder(&encoder));
	assert_int_equal(bnum_allocs(), baseline);
}

static void exact_counts_and_identity(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_weak_encoder weak = {.ref = {.refs = 7, .weak_refs = 3}, .encoder = &encoder};
	encoder.context.control = (struct obs_weak_object *)&weak;
	unsigned char before[sizeof(encoder)];
	memcpy(before, &encoder, sizeof(encoder));
	obs_weak_encoder_addref(&weak);
	assert_int_equal(weak.ref.weak_refs, 4);
	assert_int_equal(weak.ref.refs, 7);
	assert_ptr_equal(weak.encoder, &encoder);
	for (long i = 0; i < 3; i++) {
		assert_ptr_equal(obs_encoder_get_weak_encoder(&encoder), &weak);
		assert_int_equal(weak.ref.weak_refs, 5 + i);
		assert_int_equal(weak.ref.refs, 7);
		assert_ptr_equal(weak.encoder, &encoder);
		assert_memory_equal(before, &encoder, sizeof(encoder));
	}
	for (long i = 0; i < 4; i++) {
		obs_weak_encoder_release(&weak);
		assert_int_equal(weak.ref.weak_refs, 6 - i);
		assert_int_equal(weak.ref.refs, 7);
		assert_ptr_equal(weak.encoder, &encoder);
		assert_memory_equal(before, &encoder, sizeof(encoder));
	}
}

static void acquisition_does_not_check_expiry(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_weak_encoder weak = {.ref = {.refs = -1, .weak_refs = 0}, .encoder = NULL};
	encoder.context.control = (struct obs_weak_object *)&weak;
	/* Characterized, not endorsed: acquisition does not inspect strong refs
	 * or the control's encoder pointer. No upgrade/dereference is attempted. */
	assert_ptr_equal(obs_encoder_get_weak_encoder(&encoder), &weak);
	assert_int_equal(weak.ref.weak_refs, 1);
	assert_int_equal(weak.ref.refs, -1);
	assert_null(weak.encoder);
	obs_weak_encoder_release(&weak);
	assert_int_equal(weak.ref.weak_refs, 0);
}

static void final_release_frees_control(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	obs_weak_encoder_t *weak = bzalloc(sizeof(*weak));
	weak->ref.refs = -1;
	assert_int_equal(bnum_allocs(), baseline + 1);
	obs_weak_encoder_addref(weak);
	assert_int_equal(weak->ref.weak_refs, 1);
	obs_weak_encoder_release(weak);
	assert_int_equal(weak->ref.weak_refs, 0);
	assert_int_equal(bnum_allocs(), baseline + 1);
	obs_weak_encoder_release(weak);
	assert_int_equal(bnum_allocs(), baseline);
}

static const char *weak_name(void *data)
{
	(void)data;
	return "weak refs";
}
static void *weak_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}
static void weak_destroy(void *data)
{
	bfree(data);
}
static bool weak_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static void handles_survive_encoder_destruction(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_encoder_info info = {.id = "weak_refs_characterization",
					.type = OBS_ENCODER_VIDEO,
					.codec = "h264",
					.get_name = weak_name,
					.create = weak_create,
					.destroy = weak_destroy,
					.encode = weak_encode};
	obs_register_encoder(&info);
	obs_encoder_t *encoder = obs_video_encoder_create(info.id, "weak refs", NULL, NULL);
	assert_non_null(encoder);
	obs_weak_encoder_t *a = obs_encoder_get_weak_encoder(encoder);
	obs_weak_encoder_t *b = obs_encoder_get_weak_encoder(encoder);
	assert_ptr_equal(a, b);
	assert_ptr_equal(a, encoder->context.control);
	assert_int_equal(a->ref.refs, 0);
	assert_int_equal(a->ref.weak_refs, 2);
	obs_encoder_release(encoder);
	assert_int_equal(a->ref.refs, -1);
	assert_int_equal(a->ref.weak_refs, 1);
	assert_null(obs_weak_encoder_get_encoder(a));
	obs_weak_encoder_release(a);
	assert_int_equal(b->ref.weak_refs, 0);
	assert_null(obs_weak_encoder_get_encoder(b));
	long baseline = bnum_allocs();
	obs_weak_encoder_release(b);
	assert_int_equal(bnum_allocs(), baseline - 1);
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_handles), cmocka_unit_test(exact_counts_and_identity),
					   cmocka_unit_test(acquisition_does_not_check_expiry),
					   cmocka_unit_test(final_release_frees_control),
					   cmocka_unit_test(handles_survive_encoder_destruction)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
