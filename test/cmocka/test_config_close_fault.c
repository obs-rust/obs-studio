#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <cmocka.h>

#include <util/base.h>
#include <util/config-file.h>

void config_close_fault_arm(const char *path);
int config_close_fault_attempts(void);
int config_close_fault_disarm(void);

struct captured_log {
	config_t *config;
	int count;
	int level;
	char message[256];
	bool reentered;
};

static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct captured_log *log = param;
	++log->count;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Log callbacks may read the config again. */
	log->reentered = strcmp(config_get_string(log->config, "S", "x"), "new") == 0;
}

static void direct_close_failure(void **state)
{
	UNUSED_PARAMETER(state);

	char dir[] = "close-direct-XXXXXX";
	char path[128];
	assert_non_null(mkdtemp(dir));
	snprintf(path, sizeof(path), "%s/settings.ini", dir);

	config_t *config = config_create(path);
	assert_non_null(config);
	config_set_string(config, "S", "x", "new");
	assert_int_equal(config_save(config), CONFIG_SUCCESS);

	log_handler_t previous_handler;
	void *previous_param;
	base_get_log_handler(&previous_handler, &previous_param);
	struct captured_log log = {.config = config};
	base_set_log_handler(capture_log, &log);
	config_close_fault_arm(path);
	int ret = config_save(config);
	int attempts = config_close_fault_attempts();
	int count = config_close_fault_disarm();
	base_set_log_handler(previous_handler, previous_param);
	config_close(config);

	unlink(path);
	rmdir(dir);

	assert_int_equal(count, 1);
	assert_int_equal(attempts, 1);
	assert_int_equal(ret, CONFIG_ERROR);
	assert_int_equal(log.count, 0);
}

static void safe_close_failure_preserves_original(void **state)
{
	UNUSED_PARAMETER(state);

	char dir[] = "close-safe-XXXXXX";
	char path[128], temp[132], backup[132];
	assert_non_null(mkdtemp(dir));
	snprintf(path, sizeof(path), "%s/settings.ini", dir);
	snprintf(temp, sizeof(temp), "%s.tmp", path);
	snprintf(backup, sizeof(backup), "%s.bak", path);

	config_t *config = config_create(path);
	assert_non_null(config);
	config_set_string(config, "S", "x", "old");
	assert_int_equal(config_save(config), CONFIG_SUCCESS);
	config_set_string(config, "S", "x", "new");

	log_handler_t previous_handler;
	void *previous_param;
	base_get_log_handler(&previous_handler, &previous_param);
	struct captured_log log = {.config = config};
	base_set_log_handler(capture_log, &log);
	config_close_fault_arm(temp);
	int ret = config_save_safe(config, "tmp", "bak");
	int attempts = config_close_fault_attempts();
	int count = config_close_fault_disarm();
	base_set_log_handler(previous_handler, previous_param);
	config_close(config);

	char bytes[64] = {0};
	FILE *f = fopen(path, "rb");
	assert_non_null(f);
	size_t size = fread(bytes, 1, sizeof(bytes) - 1, f);
	fclose(f);

	bool backup_exists = access(backup, F_OK) == 0;
	bool temp_exists = access(temp, F_OK) == 0;
	unlink(temp);
	unlink(backup);
	unlink(path);
	rmdir(dir);

	assert_int_equal(count, 1);
	assert_int_equal(attempts, 1);
	assert_int_equal(ret, CONFIG_ERROR);
	assert_int_equal(size, strlen("[S]\nx=old\n"));
	assert_string_equal(bytes, "[S]\nx=old\n");
	assert_false(backup_exists);
	assert_true(temp_exists);
	char expected_log[256];
	snprintf(expected_log, sizeof(expected_log), "config_save_safe: failed to write to %s", temp);
	assert_int_equal(log.count, 1);
	assert_int_equal(log.level, LOG_ERROR);
	assert_string_equal(log.message, expected_log);
	assert_true(log.reentered);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(direct_close_failure),
		cmocka_unit_test(safe_close_failure_preserves_original),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
