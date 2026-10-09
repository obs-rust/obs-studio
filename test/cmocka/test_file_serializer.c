#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <cmocka.h>

#include <util/file-serializer.h>

struct paths {
	char directory[64];
	char file[128];
	char temporary[128];
};

static int setup(void **state)
{
	struct paths *paths = calloc(1, sizeof(*paths));
	assert_non_null(paths);
	strcpy(paths->directory, "/tmp/obs-file-serializer-XXXXXX");
	assert_non_null(mkdtemp(paths->directory));
	snprintf(paths->file, sizeof(paths->file), "%s/cache.bin", paths->directory);
	snprintf(paths->temporary, sizeof(paths->temporary), "%s/cache.bin.tmp", paths->directory);
	FILE *file = fopen(paths->file, "wb");
	assert_non_null(file);
	assert_int_equal(fwrite("original", 1, 8, file), 8);
	assert_int_equal(fclose(file), 0);
	*state = paths;
	return 0;
}

static int teardown(void **state)
{
	struct paths *paths = *state;
	unlink(paths->temporary);
	unlink(paths->file);
	rmdir(paths->directory);
	free(paths);
	return 0;
}

static void assert_contents(const char *path, const char *expected)
{
	FILE *file = fopen(path, "rb");
	assert_non_null(file);
	char buffer[64];
	size_t size = fread(buffer, 1, sizeof(buffer), file);
	assert_int_equal(fclose(file), 0);
	assert_int_equal(size, strlen(expected));
	assert_memory_equal(buffer, expected, size);
}

static void safe_save_preserves_original_on_flush_failure(void **state)
{
	struct paths *paths = *state;
	assert_int_equal(symlink("/dev/full", paths->temporary), 0);
	struct serializer serializer = {0};
	assert_true(file_output_serializer_init_safe(&serializer, paths->file, "tmp"));
	assert_int_equal(s_write(&serializer, "replacement", 11), 11);
	file_output_serializer_free(&serializer);
	assert_contents(paths->file, "original");
}

static void safe_save_preserves_original_on_write_failure(void **state)
{
	struct paths *paths = *state;
	assert_int_equal(symlink("/dev/full", paths->temporary), 0);
	struct serializer serializer = {0};
	assert_true(file_output_serializer_init_safe(&serializer, paths->file, "tmp"));
	char buffer[BUFSIZ * 2] = {0};
	assert_true(s_write(&serializer, buffer, sizeof(buffer)) < sizeof(buffer));
	file_output_serializer_free(&serializer);
	assert_contents(paths->file, "original");
}

static void safe_save_preserves_original_on_rename_failure(void **state)
{
	struct paths *paths = *state;
	struct serializer serializer = {0};
	assert_true(file_output_serializer_init_safe(&serializer, paths->file, "tmp"));
	assert_int_equal(s_write(&serializer, "replacement", 11), 11);
	assert_int_equal(unlink(paths->temporary), 0);
	file_output_serializer_free(&serializer);
	assert_contents(paths->file, "original");
}

static void safe_save_replaces_original(void **state)
{
	struct paths *paths = *state;
	struct serializer serializer = {0};
	assert_true(file_output_serializer_init_safe(&serializer, paths->file, "tmp"));
	assert_int_equal(s_write(&serializer, "replacement", 11), 11);
	file_output_serializer_free(&serializer);
	assert_contents(paths->file, "replacement");
	assert_int_equal(access(paths->temporary, F_OK), -1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(safe_save_preserves_original_on_flush_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(safe_save_preserves_original_on_write_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(safe_save_preserves_original_on_rename_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(safe_save_replaces_original, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
