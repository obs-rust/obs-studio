#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <cmocka.h>

#include <util/config-file.h>

static void config_save_reports_flush_failure(void **state)
{
	UNUSED_PARAMETER(state);
	config_t *config = config_create("/dev/full");
	assert_non_null(config);
	config_set_string(config, "General", "Name", "new settings");
	int result = config_save(config);
	config_close(config);
	assert_int_equal(result, CONFIG_ERROR);
}

static void config_save_safe_preserves_original(void **state)
{
	UNUSED_PARAMETER(state);
	char directory[] = "/tmp/obs-config-save-XXXXXX";
	assert_non_null(mkdtemp(directory));
	char path[128], temporary[128], backup[128];
	snprintf(path, sizeof(path), "%s/settings.ini", directory);
	snprintf(temporary, sizeof(temporary), "%s/settings.ini.tmp", directory);
	snprintf(backup, sizeof(backup), "%s/settings.ini.bak", directory);

	config_t *config = config_create(path);
	assert_non_null(config);
	config_set_string(config, "General", "Name", "original settings");
	assert_int_equal(config_save(config), CONFIG_SUCCESS);
	config_set_string(config, "General", "Name", "new settings");

	/* Small writes are buffered; /dev/full rejects the flush at fclose. */
	assert_int_equal(symlink("/dev/full", temporary), 0);
	int result = config_save_safe(config, "tmp", "bak");
	config_close(config);

	config_t *saved = NULL;
	int opened = config_open(&saved, path, CONFIG_OPEN_EXISTING);
	char name[64] = "";
	if (opened == CONFIG_SUCCESS) {
		const char *value = config_get_string(saved, "General", "Name");
		snprintf(name, sizeof(name), "%s", value ? value : "");
		config_close(saved);
	}
	bool backup_exists = access(backup, F_OK) == 0;
	unlink(temporary);
	unlink(backup);
	unlink(path);
	rmdir(directory);

	assert_int_equal(result, CONFIG_ERROR);
	assert_int_equal(opened, CONFIG_SUCCESS);
	assert_string_equal(name, "original settings");
	assert_false(backup_exists);
}

static void config_save_empty_succeeds(void **state)
{
	UNUSED_PARAMETER(state);
	char directory[] = "/tmp/obs-config-empty-XXXXXX";
	assert_non_null(mkdtemp(directory));
	char path[128], temporary[128];
	snprintf(path, sizeof(path), "%s/settings.ini", directory);
	snprintf(temporary, sizeof(temporary), "%s/settings.ini.tmp", directory);
	config_t *config = config_create(path);
	assert_non_null(config);
	int direct = config_save(config);
	int safe = config_save_safe(config, "tmp", NULL);
	config_close(config);
	unlink(temporary);
	unlink(path);
	rmdir(directory);
	assert_int_equal(direct, CONFIG_SUCCESS);
	assert_int_equal(safe, CONFIG_SUCCESS);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(config_save_reports_flush_failure),
		cmocka_unit_test(config_save_safe_preserves_original),
		cmocka_unit_test(config_save_empty_succeeds),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
