#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <cmocka.h>

#include <obs-data.h>
#include <util/platform.h>

static void quick_write_reports_flush_failure(void **state)
{
	UNUSED_PARAMETER(state);
	assert_false(os_quick_write_utf8_file("/dev/full", "settings", 8, false));
	assert_false(os_quick_write_utf8_file("/dev/full", "settings", 8, true));
	assert_false(os_quick_write_mbs_file("/dev/full", "settings", 8));
	char bytes[BUFSIZ * 2];
	memset(bytes, 'x', sizeof(bytes));
	assert_false(os_quick_write_mbs_file("/dev/full", bytes, sizeof(bytes)));
}

static void scene_save_preserves_original_on_flush_failure(void **state)
{
	UNUSED_PARAMETER(state);
	char directory[] = "/tmp/obs-scene-save-XXXXXX";
	assert_non_null(mkdtemp(directory));
	char path[128], temporary[128], backup[128];
	snprintf(path, sizeof(path), "%s/scenes.json", directory);
	snprintf(temporary, sizeof(temporary), "%s/scenes.json.tmp", directory);
	snprintf(backup, sizeof(backup), "%s/scenes.json.bak", directory);
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "name", "original scenes");
	assert_true(obs_data_save_json(data, path));
	obs_data_set_string(data, "name", "new scenes");
	assert_int_equal(symlink("/dev/full", temporary), 0);
	bool success = obs_data_save_json_safe(data, path, "tmp", "bak");
	obs_data_release(data);

	obs_data_t *saved = obs_data_create_from_json_file(path);
	char name[64] = "";
	if (saved) {
		snprintf(name, sizeof(name), "%s", obs_data_get_string(saved, "name"));
		obs_data_release(saved);
	}
	bool backup_exists = access(backup, F_OK) == 0;
	unlink(temporary);
	unlink(backup);
	unlink(path);
	rmdir(directory);
	assert_false(success);
	assert_string_equal(name, "original scenes");
	assert_false(backup_exists);
}

static void quick_write_succeeds(void **state)
{
	UNUSED_PARAMETER(state);
	char directory[] = "/tmp/obs-quick-write-XXXXXX";
	assert_non_null(mkdtemp(directory));
	char path[128];
	snprintf(path, sizeof(path), "%s/settings.txt", directory);
	assert_true(os_quick_write_utf8_file(path, "settings", 8, true));
	FILE *file = fopen(path, "rb");
	assert_non_null(file);
	char bytes[16];
	size_t size = fread(bytes, 1, sizeof(bytes), file);
	assert_int_equal(fclose(file), 0);
	assert_int_equal(size, 11);
	assert_memory_equal(bytes,
			    "\xEF\xBB\xBF"
			    "settings",
			    11);
	assert_true(os_quick_write_utf8_file(path, "", 0, false));
	assert_int_equal(os_get_file_size(path), 0);
	assert_true(os_quick_write_mbs_file(path, "settings", 8));
	assert_int_equal(os_get_file_size(path), 8);
	unlink(path);
	rmdir(directory);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(quick_write_reports_flush_failure),
		cmocka_unit_test(scene_save_preserves_original_on_flush_failure),
		cmocka_unit_test(quick_write_succeeds),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
