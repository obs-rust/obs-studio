#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include <util/pipe.h>

struct pipe_result {
	bool created;
	int status;
	size_t len;
	char payload[128];
};

static os_process_pipe_t *create_pipe(const char *cmd, os_process_args_t *args, const char *mode)
{
	return args ? os_process_pipe_create2(args, mode) : os_process_pipe_create(cmd, mode);
}

/* In read mode C replaces stdout and stderr, leaving stdin inherited. */
static struct pipe_result read_with_stdin(const char *cmd, os_process_args_t *args)
{
	struct pipe_result out = {.status = -1};
	int saved_stdin = dup(STDIN_FILENO);
	int fds[2];
	assert_true(saved_stdin >= 0);
	assert_int_equal(pipe(fds), 0);
	assert_int_equal(write(fds[1], "IN", 2), 2);
	close(fds[1]);
	assert_int_equal(dup2(fds[0], STDIN_FILENO), STDIN_FILENO);
	close(fds[0]);

	os_process_pipe_t *pp = create_pipe(cmd, args, "r");
	assert_int_equal(dup2(saved_stdin, STDIN_FILENO), STDIN_FILENO);
	close(saved_stdin);

	out.created = pp != NULL;
	if (pp) {
		while (out.len + 1 < sizeof(out.payload)) {
			size_t n = os_process_pipe_read(pp, (uint8_t *)out.payload + out.len,
							sizeof(out.payload) - 1 - out.len);
			if (!n)
				break;
			out.len += n;
		}
		out.status = os_process_pipe_destroy(pp);
	}
	out.payload[out.len] = 0;
	return out;
}

/* In write mode C replaces stdin and stderr, leaving stdout inherited. */
static struct pipe_result write_with_stdout(const char *cmd, os_process_args_t *args)
{
	struct pipe_result out = {.status = -1};
	assert_int_equal(fflush(stdout), 0);
	int saved_stdout = dup(STDOUT_FILENO);
	int fds[2];
	assert_true(saved_stdout >= 0);
	assert_int_equal(pipe(fds), 0);
	assert_int_equal(dup2(fds[1], STDOUT_FILENO), STDOUT_FILENO);

	os_process_pipe_t *pp = create_pipe(cmd, args, "w");
	assert_int_equal(dup2(saved_stdout, STDOUT_FILENO), STDOUT_FILENO);
	close(saved_stdout);
	close(fds[1]);

	out.created = pp != NULL;
	if (pp)
		out.status = os_process_pipe_destroy(pp);
	/* Destruction reaps the child, so EOF proves all inherited writers closed. */
	while (out.len + 1 < sizeof(out.payload)) {
		ssize_t n = read(fds[0], out.payload + out.len, sizeof(out.payload) - 1 - out.len);
		assert_true(n >= 0);
		if (!n)
			break;
		out.len += (size_t)n;
	}
	close(fds[0]);
	out.payload[out.len] = 0;
	return out;
}

static void assert_payload(struct pipe_result got, const char *want)
{
	assert_true(got.created);
	assert_int_equal(got.status, 0);
	assert_int_equal(got.len, strlen(want));
	assert_string_equal(got.payload, want);
}

static void test_create_read_mode_inherits_stdin(void **state)
{
	UNUSED_PARAMETER(state);
	assert_payload(read_with_stdin("/bin/cat", NULL), "IN");
}

static void test_create2_read_mode_inherits_stdin(void **state)
{
	UNUSED_PARAMETER(state);
	os_process_args_t *args = os_process_args_create("/bin/cat");
	assert_non_null(args);
	struct pipe_result got = read_with_stdin(NULL, args);
	os_process_args_destroy(args);
	assert_payload(got, "IN");
}

static void test_create_write_mode_inherits_stdout(void **state)
{
	UNUSED_PARAMETER(state);
	assert_payload(write_with_stdout("printf OUT", NULL), "OUT");
}

static void test_create2_write_mode_inherits_stdout(void **state)
{
	UNUSED_PARAMETER(state);
	os_process_args_t *args = os_process_args_create("/usr/bin/printf");
	assert_non_null(args);
	os_process_args_add_arg(args, "OUT");
	struct pipe_result got = write_with_stdout(NULL, args);
	os_process_args_destroy(args);
	assert_payload(got, "OUT");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_create_read_mode_inherits_stdin),
		cmocka_unit_test(test_create2_read_mode_inherits_stdin),
		cmocka_unit_test(test_create_write_mode_inherits_stdout),
		cmocka_unit_test(test_create2_write_mode_inherits_stdout),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
