/* C-source characterization of the hidden service info-table and plugin-data
 * accessors (#3840). They are declared only in obs-internal.h and are not
 * exported from libobs. Compile the real extracted implementation; this
 * fixture does not test a future Rust replacement until that port updates
 * its link registration. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#ifndef OBS_SERVICE_INFO_ACCESSORS_RUST
#include "../../libobs/obs-service-info-accessors.c.h"
#endif
/* Otherwise a later Rust-static registration links the four accessors,
 * declared in obs-internal.h, without editing this test. */

static unsigned log_count;

static void count_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_count++;
}

/* Every byte outside [offset, offset + size) is unchanged. */
static void assert_rest_equal(const struct obs_service *service, const struct obs_service *before, size_t offset,
			      size_t size)
{
	const unsigned char *now = (const unsigned char *)service;
	const unsigned char *old = (const unsigned char *)before;
	assert_memory_equal(now, old, offset);
	assert_memory_equal(now + offset + size, old + offset + size, sizeof(*service) - offset - size);
}

static void fill(void *dst, unsigned char seed, size_t size)
{
	unsigned char *bytes = dst;
	for (size_t i = 0; i < size; i++)
		bytes[i] = (unsigned char)(seed + i * 7u);
}

/* set_info copies the whole table by value into service->info and touches
 * nothing else; get_info_table borrows the embedded table, not the source. */
static void info_table_copy_and_borrow(void **state)
{
	(void)state;
	struct obs_service service, before;
	/* The table holds only pointers, so it has no padding to compare. Its
	 * callbacks are byte patterns that are never called. */
	struct obs_service_info first, second, first_copy;
	fill(&service, 0x11, sizeof(service));
	fill(&first, 0x5a, sizeof(first));
	fill(&second, 0xc3, sizeof(second));
	memcpy(&first_copy, &first, sizeof(first));
	log_count = 0;

	memcpy(&before, &service, sizeof(service));
	obs_service_set_info(&service, &first);
	assert_memory_equal(&service.info, &first, sizeof(first));
	assert_memory_equal(&first, &first_copy, sizeof(first));
	assert_rest_equal(&service, &before, offsetof(struct obs_service, info), sizeof(service.info));

	/* The service keeps its own copy: a later change to the source does
	 * not reach it. */
	fill(&first, 0x99, sizeof(first));
	assert_memory_equal(&service.info, &first_copy, sizeof(first_copy));

	memcpy(&before, &service, sizeof(service));
	const struct obs_service_info *table = obs_service_get_info_table(&service);
	assert_ptr_equal(table, &service.info);
	assert_ptr_not_equal(table, &first_copy);
	assert_ptr_equal(obs_service_get_info_table(&service), table);
	assert_memory_equal(&service, &before, sizeof(service));

	/* A second set overwrites the whole table in place. */
	obs_service_set_info(&service, &second);
	assert_memory_equal(&service.info, &second, sizeof(second));
	assert_rest_equal(&service, &before, offsetof(struct obs_service, info), sizeof(service.info));
	assert_ptr_equal(obs_service_get_info_table(&service), table);

	/* An all-NULL table is copied the same way. */
	struct obs_service_info empty = {0};
	obs_service_set_info(&service, &empty);
	assert_memory_equal(&service.info, &empty, sizeof(empty));
	assert_null(obs_service_get_info_table(&service)->id);
	assert_int_equal(log_count, 0);
}

/* set_plugin_data stores the pointer as-is in context.data, NULL included;
 * get_plugin_data returns it without touching the service. */
static void plugin_data_round_trip(void **state)
{
	(void)state;
	struct obs_service service, before;
	_Alignas(long double) unsigned char token_a = 0, token_b = 0;
	void *values[] = {&token_a, NULL, &token_b, &token_b};
	fill(&service, 0x27, sizeof(service));
	log_count = 0;

	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		memcpy(&before, &service, sizeof(service));
		obs_service_set_plugin_data(&service, values[i]);
		assert_ptr_equal(service.context.data, values[i]);
		assert_rest_equal(&service, &before, offsetof(struct obs_service, context.data),
				  sizeof(service.context.data));

		memcpy(&before, &service, sizeof(service));
		assert_ptr_equal(obs_service_get_plugin_data(&service), values[i]);
		assert_ptr_equal(obs_service_get_plugin_data(&service), values[i]);
		assert_memory_equal(&service, &before, sizeof(service));
	}
	/* The tokens are borrowed; neither accessor dereferences them. */
	assert_int_equal(token_a, 0);
	assert_int_equal(token_b, 0);
	assert_int_equal(log_count, 0);
}

int main(void)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(count_log, NULL);
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(info_table_copy_and_borrow),
		cmocka_unit_test(plugin_data_round_trip),
	};
	int failed = cmocka_run_group_tests(tests, NULL, NULL);
	base_set_log_handler(old_handler, old_param);
	return failed;
}
