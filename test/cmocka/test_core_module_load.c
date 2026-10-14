/*
 * Core modules that cannot be loaded must not take the others down with them.
 *
 * obs_load_core_modules() looks for every module in the core list next to the
 * executable ("<exe dir>/../" OBS_PLUGIN_PATH "/<name>.so"). Each case builds
 * that layout in a temporary directory with a copy of this test binary and
 * stub modules, runs the copy as a child and reads back what it loaded:
 *
 * - all_present: every core module is there and loads (control).
 * - missing_dependency: one core module is installed but a shared library it
 *   needs is not (an obs-browser.so without libcef.so): the others still load,
 *   the broken one is recorded as a disabled module (failed to open), and the load
 *   does not fail as a whole (OBS refused to start: obs-rust/obs-studio#404).
 * - missing_file: one core module is not installed at all (left out by a
 *   package split): every other core module still loads.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <obsconfig.h>
#include <obs-core-modules.h>
#include <util/dstr.h>
#include <util/platform.h>

#define SKIP_CODE 77

/* ------------------------------------------------------------------ child */

static int run_child(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		printf("startup-failed\n");
		return SKIP_CODE;
	}

	bool ok = obs_load_core_modules();
	printf("result %d\n", ok ? 1 : 0);

	for (unsigned int i = 0; i < obs_core_modules_count; i++) {
		const char *name = obs_core_modules[i];
		printf("module %s loaded=%d disabled=%d\n", name, obs_get_module(name) ? 1 : 0,
		       obs_get_disabled_module(name) ? 1 : 0);
	}
	fflush(stdout);

	obs_shutdown();
	return 0;
}

/* ----------------------------------------------------------------- parent */

enum scenario { ALL_PRESENT, MISSING_DEPENDENCY, MISSING_FILE };

struct child_result {
	bool skipped;
	int load_result;
	bool loaded[256];
	bool disabled[256];
};

static bool copy_file(const char *from, const char *to, mode_t mode)
{
	int in = open(from, O_RDONLY);
	if (in < 0)
		return false;
	int out = open(to, O_WRONLY | O_CREAT | O_TRUNC, mode);
	if (out < 0) {
		close(in);
		return false;
	}

	char buf[65536];
	ssize_t n;
	bool ok = true;
	while ((n = read(in, buf, sizeof(buf))) > 0) {
		if (write(out, buf, (size_t)n) != n) {
			ok = false;
			break;
		}
	}
	if (n < 0)
		ok = false;
	close(in);
	close(out);
	return ok;
}

static int index_of(const char *name)
{
	for (unsigned int i = 0; i < obs_core_modules_count; i++)
		if (strcmp(obs_core_modules[i], name) == 0)
			return (int)i;
	return -1;
}

static void parse_child_output(FILE *f, struct child_result *res)
{
	char line[1024];
	while (fgets(line, sizeof(line), f)) {
		char name[512];
		int loaded, failed;
		if (strncmp(line, "startup-failed", 14) == 0) {
			res->skipped = true;
		} else if (sscanf(line, "result %d", &res->load_result) == 1) {
			continue;
		} else if (sscanf(line, "module %511s loaded=%d disabled=%d", name, &loaded, &failed) == 3) {
			int i = index_of(name);
			if (i >= 0) {
				res->loaded[i] = loaded != 0;
				res->disabled[i] = failed != 0;
			}
		} else {
			/* The child's log, passed through. */
			fputs(line, stderr);
		}
	}
}

static void rm_rf(const char *path)
{
	char *argv[] = {"rm", "-rf", (char *)path, NULL};
	pid_t pid = fork();
	if (pid == 0) {
		execvp("rm", argv);
		_exit(127);
	}
	if (pid > 0)
		waitpid(pid, NULL, 0);
}

/* The broken module is the first one in the list, so that every other core
 * module comes after it. */
static void run_scenario(enum scenario scenario, struct child_result *res)
{
	memset(res, 0, sizeof(*res));
	res->load_result = -1;

	const char *tmp = getenv("TMPDIR");
	struct dstr root = {0};
	dstr_printf(&root, "%s/obs-core-load-XXXXXX", tmp && *tmp ? tmp : "/tmp");
	assert_non_null(mkdtemp(root.array));

	struct dstr bin_dir = {0}, exe = {0}, mod_dir = {0};
	dstr_printf(&bin_dir, "%s/bin", root.array);
	dstr_printf(&exe, "%s/test_core_module_load", bin_dir.array);
	dstr_printf(&mod_dir, "%s/" OBS_PLUGIN_PATH, root.array);
	assert_int_equal(os_mkdirs(bin_dir.array), MKDIR_SUCCESS);
	assert_int_equal(os_mkdirs(mod_dir.array), MKDIR_SUCCESS);

	char self[4096];
	ssize_t len = readlink("/proc/self/exe", self, sizeof(self) - 1);
	assert_true(len > 0);
	self[len] = '\0';
	assert_true(copy_file(self, exe.array, 0755));

	for (unsigned int i = 0; i < obs_core_modules_count; i++) {
		const char *stub = CORE_STUB_PATH;
		if (i == 0 && scenario == MISSING_FILE)
			continue;
		if (i == 0 && scenario == MISSING_DEPENDENCY)
			stub = CORE_STUB_NEEDS_DEP_PATH;

		struct dstr dst = {0};
		dstr_printf(&dst, "%s/%s.so", mod_dir.array, obs_core_modules[i]);
		assert_true(copy_file(stub, dst.array, 0755));
		dstr_free(&dst);
	}

	int fds[2];
	assert_int_equal(pipe(fds), 0);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		dup2(fds[1], STDOUT_FILENO);
		close(fds[0]);
		close(fds[1]);
		/* The dependency must stay out of reach, as libcef.so was. */
		unsetenv("LD_LIBRARY_PATH");
		char *argv[] = {exe.array, "--child", NULL};
		execv(exe.array, argv);
		_exit(127);
	}
	close(fds[1]);
	FILE *f = fdopen(fds[0], "r");
	parse_child_output(f, res);
	fclose(f);

	int status = 0;
	waitpid(pid, &status, 0);
	if (WIFEXITED(status) && WEXITSTATUS(status) == SKIP_CODE)
		res->skipped = true;
	else
		assert_true(WIFEXITED(status) && WEXITSTATUS(status) == 0);

	rm_rf(root.array);
	dstr_free(&mod_dir);
	dstr_free(&exe);
	dstr_free(&bin_dir);
	dstr_free(&root);
}

static void check_others_loaded(const struct child_result *res)
{
	for (unsigned int i = 1; i < obs_core_modules_count; i++) {
		if (!res->loaded[i])
			print_error("core module '%s' was not loaded\n", obs_core_modules[i]);
		assert_true(res->loaded[i]);
	}
}

static void all_present(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res;
	run_scenario(ALL_PRESENT, &res);
	if (res.skipped)
		skip();

	assert_int_equal(res.load_result, 1);
	assert_true(res.loaded[0]);
	check_others_loaded(&res);
}

static void missing_dependency(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res;
	run_scenario(MISSING_DEPENDENCY, &res);
	if (res.skipped)
		skip();

	/* OBS must still start: the frontend aborts when this is false. */
	assert_int_equal(res.load_result, 1);
	assert_false(res.loaded[0]);
	/* Listed as a module that could not be loaded, as plugins are. */
	assert_true(res.disabled[0]);
	check_others_loaded(&res);
}

static void missing_file(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res;
	run_scenario(MISSING_FILE, &res);
	if (res.skipped)
		skip();

	assert_int_equal(res.load_result, 1);
	assert_false(res.loaded[0]);
	check_others_loaded(&res);
}

int main(int argc, char **argv)
{
	if (argc > 1 && strcmp(argv[1], "--child") == 0)
		return run_child();

	if (obs_core_modules_count < 2 || obs_core_modules_count > 256) {
		printf("need 2..256 core modules, have %u\n", obs_core_modules_count);
		return SKIP_CODE;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(all_present),
		cmocka_unit_test(missing_dependency),
		cmocka_unit_test(missing_file),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
