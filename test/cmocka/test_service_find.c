/* C-source characterization of hidden find_service (#5267).
 * Real private layouts and controlled C-owned storage pin first-match order.
 * A future Rust swap must relink these unchanged assertions through a real
 * registry adapter; this source-only mode does not exercise a Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_core service_find_core;
struct obs_core *service_find_test_core = &service_find_core;

#ifndef SERVICE_FIND_EXTERNAL
#define obs service_find_test_core
#include "libobs/obs-service-find.c.h"
#undef obs
#endif

static unsigned callback_count;

static const char *service_find_name(void *data)
{
	(void)data;
	callback_count++;
	return "unused";
}

static void *service_find_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	callback_count++;
	return NULL;
}

static void service_find_destroy(void *data)
{
	(void)data;
	callback_count++;
}

static void check_lookup(const char *id, const struct obs_service_info *expected)
{
	struct obs_service_info before[8];
	unsigned char core_before[sizeof(service_find_core)];
	assert_true(service_find_core.service_types.capacity <= 8);
	size_t bytes = service_find_core.service_types.capacity * sizeof(before[0]);
	if (bytes)
		memcpy(before, service_find_core.service_types.array, bytes);
	memcpy(core_before, &service_find_core, sizeof(core_before));
	long allocations = bnum_allocs();
	callback_count = 0;

	const struct obs_service_info *actual = find_service(id);

	assert_ptr_equal(actual, expected);
	assert_int_equal(callback_count, 0);
	assert_int_equal(bnum_allocs(), allocations);
	assert_memory_equal(&service_find_core, core_before, sizeof(core_before));
	if (bytes)
		assert_memory_equal(service_find_core.service_types.array, before, bytes);
	if (expected) {
		assert_ptr_equal(actual->id, expected->id);
		assert_ptr_equal(actual->type_data, expected->type_data);
	}
}

static void registry_lookup(void **state)
{
	(void)state;
	memset(&service_find_core, 0, sizeof(service_find_core));
	check_lookup("", NULL);
	check_lookup("missing", NULL);

	char first_id[] = "svc";
	char second_id[] = "svc";
	char query[] = "svc";
	char binary_id[] = {(char)0xff, (char)0x80, 'x', 0};
	char binary_query[] = {(char)0xff, (char)0x80, 'x', 0};
	char binary_miss[] = {(char)0xff, (char)0x81, 'x', 0};
	int owners[8] = {0};
	struct obs_service_info entries[8] = {0};
	const char *ids[] = {first_id, "svc.suffix", "SVC", "", binary_id, second_id, "last", "outside"};
	for (size_t i = 0; i < 8; i++) {
		entries[i].id = ids[i];
		entries[i].get_name = service_find_name;
		entries[i].create = service_find_create;
		entries[i].destroy = service_find_destroy;
		entries[i].type_data = &owners[i];
		entries[i].free_type_data = service_find_destroy;
	}
	service_find_core.service_types.array = entries;
	service_find_core.service_types.capacity = 8;
	/* Allocated storage is not a logical entry when num is zero. */
	check_lookup(query, NULL);
	service_find_core.service_types.num = 7;
	check_lookup(query, &entries[0]);
	check_lookup("sv", NULL);
	check_lookup("svc.suffix", &entries[1]);
	check_lookup("svc.suffix.more", NULL);
	check_lookup("SVC", &entries[2]);
	check_lookup("Svc", NULL);
	check_lookup("", &entries[3]);
	check_lookup(binary_query, &entries[4]);
	check_lookup(binary_miss, NULL);
	check_lookup("last", &entries[6]);
	check_lookup("outside", NULL);
	check_lookup("missing", NULL);

	/* Characterized, not endorsed: duplicate IDs select the FIRST entry.
	 * Moving/removing that entry must immediately affect the borrowed result. */
	struct obs_service_info swap = entries[0];
	entries[0] = entries[5];
	entries[5] = swap;
	check_lookup(query, &entries[0]);
	entries[0].id = "replaced";
	check_lookup(query, &entries[5]);
	assert_memory_equal(first_id, "svc", sizeof(first_id));
	assert_memory_equal(second_id, "svc", sizeof(second_id));
	assert_memory_equal(owners, (int[8]){0}, sizeof(owners));
	memset(&service_find_core, 0, sizeof(service_find_core));
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(registry_lookup)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
