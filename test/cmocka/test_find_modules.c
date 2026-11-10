/* Characterize discovery before extraction. Callbacks borrow their info; copy
 * values during the call. The legacy API exposes the info2 binary/data prefix
 * by callback cast, without copying or changing the parameter. NULL callbacks
 * are safe only when no module is found; calling NULL for a discovered module
 * is outside the valid domain (characterized, not endorsed), so not executed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

struct fixture {
	char root[128];
	char bins[3][256];
	char files[3][512];
	char data[256];
	size_t count;
	bool append;
};

static const char *names[] = {"z", "a", "m"};

static int setup(void **state)
{
	struct fixture *f = calloc(1, sizeof(*f));
	assert_non_null(f);
	strcpy(f->root, "/tmp/obs-find-modules-XXXXXX");
	assert_non_null(mkdtemp(f->root));
	snprintf(f->data, sizeof(f->data), "%s/data", f->root);
	assert_int_equal(mkdir(f->data, 0700), 0);
	for (size_t i = 0; i < 3; i++) {
		snprintf(f->bins[i], sizeof(f->bins[i]), "%s/bin-%s", f->root, names[i]);
		assert_int_equal(mkdir(f->bins[i], 0700), 0);
		snprintf(f->files[i], sizeof(f->files[i]), "%s/%s.so", f->bins[i], names[i]);
		FILE *file = fopen(f->files[i], "wb");
		assert_non_null(file);
		assert_int_equal(fclose(file), 0);
	}
	*state = f;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	obs_shutdown();
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(remove(f->files[i]), 0);
		assert_int_equal(rmdir(f->bins[i]), 0);
	}
	assert_int_equal(rmdir(f->data), 0);
	assert_int_equal(rmdir(f->root), 0);
	free(f);
	return 0;
}

static void collect(void *param, const struct obs_module_info2 *info)
{
	struct fixture *f = param;
	assert_non_null(f);
	assert_true(f->count < 3);
	size_t i = f->count++;
	assert_string_equal(info->bin_path, f->files[i]);
	assert_string_equal(info->data_path, f->data);
	assert_string_equal(info->name, names[i]);
	assert_int_equal(info->type, OBS_MODULE_TYPE_UNDEFINED);
	/* The traversal reads the live path count after each callback. */
	if (f->append && i == 0)
		obs_add_module_path(f->bins[2], f->data);
}

static void collect_legacy(void *param, const struct obs_module_info *info)
{
	struct fixture *f = param;
	assert_non_null(f);
	assert_true(f->count < 3);
	size_t i = f->count++;
	assert_string_equal(info->bin_path, f->files[i]);
	assert_string_equal(info->data_path, f->data);
}

static void absent_core(void **state)
{
	(void)state;
	assert_false(obs_initialized());
	struct fixture f = {0};
	obs_find_modules2(collect, &f);
	obs_find_modules(collect_legacy, &f);
	obs_find_modules2(NULL, NULL);
	obs_find_modules(NULL, NULL);
	assert_int_equal(f.count, 0);
}

static void empty_and_missing(void **state)
{
	struct fixture *f = *state;
	obs_find_modules2(NULL, f);
	obs_find_modules(NULL, f);
	/* The existing data directory contains no shared objects. */
	obs_add_module_path(f->data, f->data);
	obs_find_modules2(NULL, f);
	obs_find_modules(NULL, f);
	obs_find_modules2(collect, f);
	obs_find_modules(collect_legacy, f);
	assert_int_equal(f->count, 0);
}

static void path_order_and_legacy_prefix(void **state)
{
	struct fixture *f = *state;
	obs_add_module_path(f->bins[0], f->data);
	obs_add_module_path(f->data, f->data);
	obs_add_module_path(f->bins[1], f->data);
	obs_add_module_path(f->bins[2], f->data);
	obs_find_modules2(collect, f);
	assert_int_equal(f->count, 3);
	f->count = 0;
	obs_find_modules(collect_legacy, f);
	assert_int_equal(f->count, 3);
}

static void appended_path_is_visited(void **state)
{
	struct fixture *f = *state;
	obs_add_module_path(f->bins[0], f->data);
	obs_add_module_path(f->bins[1], f->data);
	f->append = true;
	obs_find_modules2(collect, f);
	assert_int_equal(f->count, 3);
	f->count = 0;
	obs_find_modules(collect_legacy, f);
	assert_int_equal(f->count, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(absent_core),
		cmocka_unit_test_setup_teardown(empty_and_missing, setup, teardown),
		cmocka_unit_test_setup_teardown(path_order_and_legacy_prefix, setup, teardown),
		cmocka_unit_test_setup_teardown(appended_path_is_visited, setup, teardown),
		cmocka_unit_test(absent_core),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
