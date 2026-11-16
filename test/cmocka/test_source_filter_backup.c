/* Characterize obs_source_backup_filters through the real libobs ABI. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static obs_source_t *backup_parent;
static char save_order[8];
static size_t save_count;
static unsigned log_count;
static int log_level;
static char log_message[128];

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

struct mutex_probe {
	pthread_mutex_t *mutex;
	int result;
};

static void *try_parent_mutex(void *data)
{
	struct mutex_probe *probe = data;
	probe->result = pthread_mutex_trylock(probe->mutex);
	if (probe->result == 0)
		pthread_mutex_unlock(probe->mutex);
	return NULL;
}

static void assert_parent_mutex(int expected)
{
	/* A different thread is necessary: filter_mutex is recursive. No sleeps. */
	struct mutex_probe probe = {.mutex = &backup_parent->filter_mutex, .result = -1};
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, try_parent_mutex, &probe), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(probe.result, expected);
}

static const char *probe_name(void *unused)
{
	(void)unused;
	return "filter backup probe";
}

static void *probe_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void probe_destroy(void *data)
{
	(void)data;
}

static void probe_save(void *data, obs_data_t *settings)
{
	obs_source_t *source = data;
	assert_parent_mutex(EBUSY);
	assert_true(save_count + 1 < sizeof(save_order));
	save_order[save_count++] = obs_source_get_name(source)[0];
	save_order[save_count] = '\0';
	obs_data_set_int(settings, "save_ordinal", (long long)save_count);
}

static const struct obs_source_info parent_info = {
	.id = "backup_parent",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = probe_name,
	.create = probe_create,
	.destroy = probe_destroy,
};

static const struct obs_source_info filter_info = {
	.id = "backup_filter",
	.type = OBS_SOURCE_TYPE_FILTER,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = probe_name,
	.create = probe_create,
	.destroy = probe_destroy,
	.save = probe_save,
};

static void null_source_is_rejected(void **unused)
{
	(void)unused;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	log_count = 0;
	base_set_log_handler(capture_log, NULL);
	obs_data_array_t *array = obs_source_backup_filters(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_null(array);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_backup_filters: Null 'source' parameter");
}

static void empty_backups_are_owned_arrays(void **unused)
{
	(void)unused;
	obs_data_array_t *first = obs_source_backup_filters(backup_parent);
	obs_data_array_t *second = obs_source_backup_filters(backup_parent);
	assert_non_null(first);
	assert_non_null(second);
	assert_ptr_not_equal(first, second);
	assert_int_equal(obs_data_array_count(first), 0);
	assert_int_equal(obs_data_array_count(second), 0);
	obs_data_t *item = obs_data_create();
	obs_data_array_push_back(first, item);
	obs_data_release(item);
	assert_int_equal(obs_data_array_count(first), 1);
	assert_int_equal(obs_data_array_count(second), 0);
	assert_int_equal(obs_source_filter_count(backup_parent), 0);
	assert_parent_mutex(0);
	obs_data_array_release(first);
	obs_data_array_release(second);
}

static void check_populated_backup(size_t count)
{
	const char *names[] = {"alpha", "beta", "gamma"};
	obs_source_t *filters[3] = {0};
	for (size_t i = 0; i < count; i++) {
		obs_data_t *settings = obs_data_create();
		obs_data_set_int(settings, "marker", (long long)(10 + i));
		filters[i] = obs_source_create_private(filter_info.id, names[i], settings);
		obs_data_release(settings);
		assert_non_null(filters[i]);
		obs_source_filter_add(backup_parent, filters[i]);
	}
	/* Add prepends. Even a disabled filter is backed up in forward list order. */
	obs_source_set_enabled(filters[0], false);
	save_count = 0;
	save_order[0] = '\0';
	obs_data_array_t *array = obs_source_backup_filters(backup_parent);
	assert_non_null(array);
	assert_int_equal(obs_data_array_count(array), count);
	assert_string_equal(save_order, count == 1 ? "a" : "gba");
	assert_int_equal(save_count, count);
	assert_parent_mutex(0);
	obs_data_t *held = obs_data_array_item(array, 0);
	assert_non_null(held);
	for (size_t i = 0; i < count; i++) {
		size_t index = count - i - 1;
		assert_ptr_equal(backup_parent->filters.array[i], filters[index]);
		obs_data_t *entry = obs_data_array_item(array, i);
		assert_non_null(entry);
		assert_string_equal(obs_data_get_string(entry, "name"), names[index]);
		assert_string_equal(obs_data_get_string(entry, "id"), filter_info.id);
		assert_string_equal(obs_data_get_string(entry, "versioned_id"), filter_info.id);
		assert_string_equal(obs_data_get_string(entry, "uuid"), obs_source_get_uuid(filters[index]));
		assert_int_equal(obs_data_get_bool(entry, "enabled"), index != 0);
		obs_data_t *settings = obs_data_get_obj(entry, "settings");
		assert_non_null(settings);
		assert_int_equal(obs_data_get_int(settings, "marker"), 10 + index);
		assert_int_equal(obs_data_get_int(settings, "save_ordinal"), i + 1);
		obs_data_release(settings);
		obs_data_release(entry);
	}
	for (size_t i = 0; i < count; i++)
		obs_source_release(filters[i]);
	obs_source_release(backup_parent);
	backup_parent = NULL;
	obs_wait_for_destroy_queue();
	/* The array owns each saved object after the sources have gone away. */
	obs_data_t *last = obs_data_array_item(array, count - 1);
	assert_string_equal(obs_data_get_string(last, "name"), "alpha");
	obs_data_release(last);
	obs_data_array_release(array);
	/* An acquired item in turn outlives the returned array. */
	assert_string_equal(obs_data_get_string(held, "name"), names[count - 1]);
	obs_data_t *settings = obs_data_get_obj(held, "settings");
	assert_int_equal(obs_data_get_int(settings, "marker"), 10 + count - 1);
	obs_data_release(settings);
	obs_data_release(held);
}

static void singleton_backup(void **unused)
{
	(void)unused;
	check_populated_backup(1);
}

static void forward_order_and_ownership(void **unused)
{
	(void)unused;
	check_populated_backup(3);
}

static void backup_releases_temporary_references(void **unused)
{
	(void)unused;
	obs_source_t *filter = obs_source_create_private(filter_info.id, "alpha", NULL);
	assert_non_null(filter);
	obs_source_filter_add(backup_parent, filter);
	save_count = 0;
	/* Warm the save callback's settings and hotkey data before measuring. */
	obs_data_array_t *array = obs_source_backup_filters(backup_parent);
	obs_data_array_release(array);
	long baseline = bnum_allocs();
	array = obs_source_backup_filters(backup_parent);
	assert_non_null(array);
	assert_int_equal(obs_data_array_count(array), 1);
	obs_data_array_release(array);
	/* push_back retains the object; backup must release its temporary ref. */
	assert_int_equal(bnum_allocs(), baseline);
	assert_parent_mutex(0);
	obs_source_release(filter);
}

static int setup(void **unused)
{
	(void)unused;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	obs_register_source(&parent_info);
	obs_register_source(&filter_info);
	backup_parent = obs_source_create_private(parent_info.id, "parent", NULL);
	return backup_parent ? 0 : -1;
}

static int teardown(void **unused)
{
	(void)unused;
	obs_source_release(backup_parent);
	backup_parent = NULL;
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_source_is_rejected, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_backups_are_owned_arrays, setup, teardown),
		cmocka_unit_test_setup_teardown(singleton_backup, setup, teardown),
		cmocka_unit_test_setup_teardown(backup_releases_temporary_references, setup, teardown),
		cmocka_unit_test_setup_teardown(forward_order_and_ownership, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
