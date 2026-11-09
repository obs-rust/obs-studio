#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <util/bmem.h>

/* Characterize the public C wrapper with a fake device, without a GPU.
 * The device owns the pointer dispatched to it, including a duplicated buffer.
 * These assertions must remain unchanged when the implementation is ported. */
static struct graphics_subsystem graphics;
static char device_token;
static struct {
	void *indices;
	enum gs_index_type type;
	size_t num;
	uint32_t flags;
} buffer;
static size_t calls;
static bool reject;

static void context_noop(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static gs_indexbuffer_t *create_buffer(gs_device_t *device, enum gs_index_type type, void *indices, size_t num,
				       uint32_t flags)
{
	assert_ptr_equal(device, &device_token);
	++calls;
	buffer.indices = indices;
	buffer.type = type;
	buffer.num = num;
	buffer.flags = flags;
	/* A failed backend consumes the input too: the wrapper does not free it. */
	if (reject) {
		bfree(indices);
		buffer.indices = NULL;
		return NULL;
	}
	return (gs_indexbuffer_t *)&buffer;
}

static void destroy_buffer(gs_indexbuffer_t *ib)
{
	assert_ptr_equal(ib, &buffer);
	bfree(buffer.indices);
	buffer.indices = NULL;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	memset(&buffer, 0, sizeof(buffer));
	calls = 0;
	reject = false;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context_noop;
	graphics.exports.device_leave_context = context_noop;
	graphics.exports.device_indexbuffer_create = create_buffer;
	graphics.exports.gs_indexbuffer_destroy = destroy_buffer;
	gs_enter_context(&graphics);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_null(buffer.indices);
	gs_leave_context();
	assert_null(gs_get_context());
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void absent_context_neither_dispatches_nor_takes_ownership(void **state)
{
	(void)state;
	void *input = bmemdup("abcd", 4);
	gs_leave_context();
	assert_null(gs_indexbuffer_create(GS_UNSIGNED_LONG, input, 1, GS_DUP_BUFFER));
	assert_null(gs_indexbuffer_create(GS_UNSIGNED_SHORT, NULL, 0, 0));
	assert_int_equal(calls, 0);
	assert_memory_equal(input, "abcd", 4);
	bfree(input);
	gs_enter_context(&graphics);
}

static void input_count_flags_and_widths(void **state)
{
	(void)state;
	const enum gs_index_type types[] = {GS_UNSIGNED_SHORT, GS_UNSIGNED_LONG};
	const uint32_t flags[] = {0, GS_DYNAMIC, GS_DUP_BUFFER, GS_DUP_BUFFER | GS_DYNAMIC};
	const unsigned char bytes[] = {0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x10, 0x32, 0x54, 0x76};
	for (size_t t = 0; t < 2; ++t) {
		for (size_t f = 0; f < 4; ++f) {
			for (size_t present = 0; present < 2; ++present) {
				for (size_t num = 0; num <= 3; num += 3) {
					size_t size = num * (types[t] == GS_UNSIGNED_SHORT ? 2 : 4);
					/* Exact-sized input lets ASan detect an oversized copy. */
					void *input = present ? bmemdup(bytes, size ? size : 1) : NULL;
					size_t before = calls;
					gs_indexbuffer_t *ib = gs_indexbuffer_create(types[t], input, num, flags[f]);
					assert_ptr_equal(ib, &buffer);
					assert_int_equal(calls, before + 1);
					assert_int_equal(buffer.type, types[t]);
					assert_int_equal(buffer.num, num);
					assert_int_equal(buffer.flags, flags[f]);
					bool duplicated = input && num && (flags[f] & GS_DUP_BUFFER);
					if (duplicated) {
						assert_ptr_not_equal(buffer.indices, input);
						assert_memory_equal(buffer.indices, bytes, size);
						memset(input, 0, size);
						assert_memory_equal(buffer.indices, bytes, size);
					} else {
						/* Null input and zero count still dispatch, even with DUP.
						 * Characterized, not endorsed. */
						assert_ptr_equal(buffer.indices, input);
					}
					gs_indexbuffer_destroy(ib);
					if (duplicated) {
						/* Backend destruction must leave the caller's copy alive. */
						assert_int_equal(((unsigned char *)input)[0], 0);
						bfree(input);
					}
				}
			}
		}
	}
}

static void backend_failure_is_propagated(void **state)
{
	(void)state;
	reject = true;
	for (size_t dup = 0; dup < 2; ++dup) {
		void *input = bmemdup("abcdefgh", 8);
		size_t before = calls;
		assert_null(gs_indexbuffer_create(GS_UNSIGNED_LONG, input, 2, dup ? GS_DUP_BUFFER : 0));
		assert_int_equal(calls, before + 1);
		if (dup) {
			assert_memory_equal(input, "abcdefgh", 8);
			bfree(input);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(absent_context_neither_dispatches_nor_takes_ownership, setup, teardown),
		cmocka_unit_test_setup_teardown(input_count_flags_and_widths, setup, teardown),
		cmocka_unit_test_setup_teardown(backend_failure_is_propagated, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
