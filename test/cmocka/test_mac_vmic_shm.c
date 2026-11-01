#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <cmocka.h>

#include "vmic-shm.h"

static struct vmic_shm *ring(uint32_t capacity)
{
	size_t size = vmic_shm_size(2, capacity);
	struct vmic_shm *shm = calloc(1, size);
	vmic_shm_init(shm, 48000, 2, capacity);
	assert_true(vmic_shm_valid(shm, size));
	return shm;
}

static void reader_rejects_a_foreign_or_short_mapping(void **state)
{
	(void)state;
	struct vmic_shm *shm = ring(4);
	assert_false(vmic_shm_valid(shm, sizeof(struct vmic_shm)));
	shm->version = VMIC_SHM_VERSION + 1;
	assert_false(vmic_shm_valid(shm, vmic_shm_size(2, 4)));
	free(shm);
}

static void frames_come_out_in_order_and_wrap(void **state)
{
	(void)state;
	struct vmic_shm *shm = ring(4);
	const float a[6] = {1, 2, 3, 4, 5, 6};
	const float b[4] = {7, 8, 9, 10};
	float out[8];

	assert_true(vmic_shm_write(shm, a, 3, 100));
	assert_int_equal(vmic_shm_read(shm, out, 2), 2);
	assert_true(vmic_shm_write(shm, b, 2, 200));
	assert_int_equal(atomic_load(&shm->write_host_time), 200);
	assert_int_equal(vmic_shm_read(shm, out, 3), 3);
	const float want[6] = {5, 6, 7, 8, 9, 10};
	assert_memory_equal(out, want, sizeof(want));
	free(shm);
}

static void packet_that_does_not_fit_is_dropped_whole(void **state)
{
	(void)state;
	struct vmic_shm *shm = ring(4);
	const float a[6] = {1, 2, 3, 4, 5, 6};
	float out[8];

	assert_true(vmic_shm_write(shm, a, 3, 1));
	assert_false(vmic_shm_write(shm, a, 2, 2));
	assert_int_equal(vmic_shm_read(shm, out, 4), 3);
	assert_memory_equal(out, a, sizeof(a));
	free(shm);
}

static void short_read_is_padded_with_silence(void **state)
{
	(void)state;
	struct vmic_shm *shm = ring(4);
	const float a[2] = {1, 2};
	float out[6] = {9, 9, 9, 9, 9, 9};

	assert_true(vmic_shm_write(shm, a, 1, 1));
	assert_int_equal(vmic_shm_read(shm, out, 3), 1);
	const float want[6] = {1, 2, 0, 0, 0, 0};
	assert_memory_equal(out, want, sizeof(want));
	free(shm);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(reader_rejects_a_foreign_or_short_mapping),
		cmocka_unit_test(frames_come_out_in_order_and_wrap),
		cmocka_unit_test(packet_that_does_not_fit_is_dropped_whole),
		cmocka_unit_test(short_read_is_padded_with_silence),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
