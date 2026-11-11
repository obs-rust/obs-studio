/* Direct characterization of encoder packet ownership before its Rust port.
 * The hidden create_instance body is compiled verbatim from production C;
 * ref/release and the allocator are called through the actual libobs library.
 * create_instance requires a non-NULL source and destination. Raw plugin data
 * has no refcount prefix and must be copied before ref/release can own it.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include <obs.h>
#include <util/bmem.h>
#include "encoder-packet-create-under-test.h"

static void assert_metadata(const struct encoder_packet *actual, const struct encoder_packet *expected)
{
	assert_int_equal(actual->size, expected->size);
	assert_int_equal(actual->pts, expected->pts);
	assert_int_equal(actual->dts, expected->dts);
	assert_int_equal(actual->timebase_num, expected->timebase_num);
	assert_int_equal(actual->timebase_den, expected->timebase_den);
	assert_int_equal(actual->type, expected->type);
	assert_int_equal(actual->keyframe, expected->keyframe);
	assert_int_equal(actual->dts_usec, expected->dts_usec);
	assert_int_equal(actual->sys_dts_usec, expected->sys_dts_usec);
	assert_int_equal(actual->priority, expected->priority);
	assert_int_equal(actual->drop_priority, expected->drop_priority);
	assert_int_equal(actual->track_idx, expected->track_idx);
	assert_ptr_equal(actual->encoder, expected->encoder);
}

static void assert_zeroed(const struct encoder_packet *packet)
{
	unsigned char zero[sizeof(*packet)] = {0};
	assert_memory_equal(packet, zero, sizeof(*packet));
}

static long references(const struct encoder_packet *packet)
{
	long count;
	memcpy(&count, packet->data - sizeof(count), sizeof(count));
	return count;
}

static void copies_raw_and_owned_payloads(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	uint8_t raw[] = {0x00, 0xff, 0x42, 0x80, 0x01};
	struct encoder_packet source = {
		.data = raw,
		.size = sizeof(raw),
		.pts = INT64_C(-12345678901),
		.dts = INT64_C(9876543210),
		.timebase_num = 1001,
		.timebase_den = 30000,
		.type = OBS_ENCODER_VIDEO,
		.keyframe = true,
		.dts_usec = -123456,
		.sys_dts_usec = 987654,
		.priority = 7,
		.drop_priority = -3,
		.track_idx = 2,
		/* Opaque identity only, never dereferenced. */
		.encoder = (obs_encoder_t *)&source,
	};
	struct encoder_packet first = {0};
	struct encoder_packet second = {0};
	packet_create_under_test(&first, &source);
	assert_metadata(&first, &source);
	assert_ptr_equal(source.data, raw);
	assert_ptr_not_equal(first.data, raw);
	assert_memory_equal(first.data, raw, sizeof(raw));
	assert_int_equal(references(&first), 1);
	assert_int_equal(bnum_allocs(), baseline + 1);

	/* Even an already owned source is independently allocated and copied. */
	packet_create_under_test(&second, &first);
	assert_metadata(&second, &source);
	assert_ptr_not_equal(second.data, first.data);
	assert_memory_equal(second.data, raw, sizeof(raw));
	assert_int_equal(references(&first), 1);
	assert_int_equal(references(&second), 1);
	assert_int_equal(bnum_allocs(), baseline + 2);
	raw[0] = 0x11;
	first.data[1] = 0x22;
	assert_int_equal(first.data[0], 0x00);
	assert_int_equal(second.data[0], 0x00);
	assert_int_equal(second.data[1], 0xff);
	assert_int_equal(raw[1], 0xff);
	obs_encoder_packet_release(&first);
	assert_zeroed(&first);
	assert_int_equal(bnum_allocs(), baseline + 1);
	assert_int_equal(second.data[1], 0xff);
	obs_encoder_packet_release(&second);
	assert_zeroed(&second);
	assert_int_equal(bnum_allocs(), baseline);
}

static void retains_aliases_until_last_release(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	uint8_t raw[] = {0x12, 0x34, 0x56};
	struct encoder_packet source = {.data = raw, .size = sizeof(raw), .pts = -4, .track_idx = 5};
	struct encoder_packet owner = {0};
	struct encoder_packet first = {0};
	struct encoder_packet second = {0};
	packet_create_under_test(&owner, &source);
	obs_encoder_packet_ref(&first, &owner);
	assert_ptr_equal(first.data, owner.data);
	assert_metadata(&first, &owner);
	assert_int_equal(references(&owner), 2);
	obs_encoder_packet_ref(&second, &first);
	assert_ptr_equal(second.data, owner.data);
	assert_metadata(&second, &owner);
	assert_int_equal(references(&owner), 3);
	assert_int_equal(bnum_allocs(), baseline + 1);
	first.data[1] = 0xab;
	assert_int_equal(owner.data[1], 0xab);
	assert_int_equal(second.data[1], 0xab);
	assert_int_equal(raw[1], 0x34);
	obs_encoder_packet_release(&owner);
	assert_zeroed(&owner);
	assert_int_equal(references(&second), 2);
	obs_encoder_packet_release(&second);
	assert_zeroed(&second);
	assert_int_equal(references(&first), 1);
	assert_int_equal(first.data[1], 0xab);
	assert_int_equal(bnum_allocs(), baseline + 1);
	obs_encoder_packet_release(&first);
	assert_zeroed(&first);
	assert_int_equal(bnum_allocs(), baseline);
	obs_encoder_packet_release(&first);
	assert_zeroed(&first);
	assert_int_equal(bnum_allocs(), baseline);
}

static void empty_payload_still_allocates(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	uint8_t sentinel = 0xa5;
	/* A valid pointer with size zero avoids relying on memcpy(NULL, ..., 0). */
	struct encoder_packet source = {.data = &sentinel, .size = 0, .pts = 42};
	struct encoder_packet owner = {0};
	struct encoder_packet alias = {0};
	packet_create_under_test(&owner, &source);
	assert_non_null(owner.data);
	assert_metadata(&owner, &source);
	assert_int_equal(references(&owner), 1);
	assert_int_equal(bnum_allocs(), baseline + 1);
	obs_encoder_packet_ref(&alias, &owner);
	assert_ptr_equal(alias.data, owner.data);
	assert_int_equal(references(&alias), 2);
	assert_int_equal(bnum_allocs(), baseline + 1);
	obs_encoder_packet_release(&alias);
	assert_zeroed(&alias);
	assert_int_equal(references(&owner), 1);
	obs_encoder_packet_release(&owner);
	assert_zeroed(&owner);
	assert_int_equal(sentinel, 0xa5);
	assert_int_equal(bnum_allocs(), baseline);
}

static void null_and_dataless_packets(void **state)
{
	(void)state;
	long baseline = bnum_allocs();
	struct encoder_packet destination;
	unsigned char unchanged[sizeof(destination)];
	memset(&destination, 0xa5, sizeof(destination));
	memcpy(unchanged, &destination, sizeof(destination));
	obs_encoder_packet_ref(&destination, NULL);
	assert_memory_equal(&destination, unchanged, sizeof(destination));
	obs_encoder_packet_ref(NULL, NULL);
	obs_encoder_packet_release(NULL);
	assert_int_equal(bnum_allocs(), baseline);

	/* No data means no prefix access, even with nonzero size and metadata.
	 * This is characterized, not endorsed as a valid encoded payload. */
	struct encoder_packet source = {.data = NULL, .size = 17, .pts = -9, .keyframe = true};
	obs_encoder_packet_ref(&destination, &source);
	assert_null(destination.data);
	assert_metadata(&destination, &source);
	assert_int_equal(bnum_allocs(), baseline);
	obs_encoder_packet_release(&destination);
	assert_zeroed(&destination);
	assert_int_equal(source.size, 17);
	obs_encoder_packet_release(&source);
	assert_zeroed(&source);
	assert_int_equal(bnum_allocs(), baseline);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(copies_raw_and_owned_payloads),
					   cmocka_unit_test(retains_aliases_until_last_release),
					   cmocka_unit_test(empty_payload_still_allocates),
					   cmocka_unit_test(null_and_dataless_packets)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
