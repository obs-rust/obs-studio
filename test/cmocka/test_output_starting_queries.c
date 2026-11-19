/* C-source characterization of the private output predicates (#5229).
 * Record atomic-load addresses, including skipped reads, using real layouts.
 * A later Rust swap must relink this fixture through real private adapters;
 * the unchanged failed-start/bind/reset tests also cover the live library. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_output output;
static const volatile bool *expected_reads[4];
static size_t expected_count, read_count;

static bool recording_load_bool(const volatile bool *ptr)
{
	assert_true(read_count < expected_count);
	assert_ptr_equal(ptr, expected_reads[read_count]);
	read_count++;
	return os_atomic_load_bool(ptr);
}

/* The three same-group helpers retain their original atomic-load behavior. */
static inline bool active(const struct obs_output *out)
{
	return recording_load_bool(&out->active);
}

static inline bool reconnecting(const struct obs_output *out)
{
	return recording_load_bool(&out->reconnecting);
}

static inline bool delay_active(const struct obs_output *out)
{
	return recording_load_bool(&out->delay_active);
}

#define os_atomic_load_bool recording_load_bool
#include "libobs/obs-output-starting-queries.c.h"
#undef os_atomic_load_bool

static void flag_matrix(void **state)
{
	(void)state;
	for (unsigned flags = 0; flags < 16; flags++) {
		memset(&output, 0, sizeof(output));
		output.active = (flags & 1) != 0;
		output.starting = (flags & 2) != 0;
		output.reconnecting = (flags & 4) != 0;
		output.delay_active = (flags & 8) != 0;
		/* Irrelevant flags must never be read, even when they are true. */
		output.delay_capturing = true;
		output.data_active = true;
		const volatile bool *fields[] = {&output.active, &output.starting, &output.reconnecting,
						 &output.delay_active};
		unsigned char before[sizeof(output)];
		memcpy(before, &output, sizeof(output));
		for (unsigned first = 0; first < 2; first++) {
			expected_count = read_count = 0;
			for (unsigned i = first; i < 4; i++) {
				expected_reads[expected_count++] = fields[i];
				if ((flags & (1U << i)) != 0)
					break;
			}
			bool actual = first == 0 ? obs_output_needs_encoders(&output) : obs_output_starting(&output);
			assert_int_equal(actual, (flags & (first == 0 ? 15U : 14U)) != 0);
			assert_int_equal(read_count, expected_count);
			assert_memory_equal(&output, before, sizeof(output));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(flag_matrix)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
