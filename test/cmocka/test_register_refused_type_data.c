/* Regression test for obs-rust/obs-studio#2195: when obs_register_source_s(),
 * obs_register_output_s(), obs_register_encoder_s() or
 * obs_register_service_s() refuses a type, HANDLE_ERROR() in
 * libobs/obs-module.c.h frees the type data with the caller's
 * free_type_data(type_data). It copied the first `size` bytes of the
 * caller's structure into a zeroed one, so a size that ends inside
 * free_type_data left a partly copied pointer, and HANDLE_ERROR() called it.
 *
 * Each type is registered through the public API with get_name missing, so
 * it is refused, at sizes that end inside type_data, inside free_type_data
 * (halfway, and one byte short), exactly at the end of free_type_data, and at
 * sizeof. free_type_data must be called only when type_data and
 * free_type_data both lie wholly within the size, and then once with the
 * caller's type_data. type_data is a byte pattern the test never
 * dereferences, so a partly copied type_data compares unequal to it.
 *
 * Each registration runs in a forked child, so a jump through a partly
 * copied pointer fails only its own case. On 64-bit Linux a pointer one byte
 * short is usually still whole (its top byte is 0), so the buggy code calls
 * free_type_data there without crashing; halfway it jumps into unmapped
 * memory. Windows has no fork(), so the cases run in the test process.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. It skips only when no display is set at all; with one set, a failed
 * obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef _WIN32
#include <errno.h>
#include <signal.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif
#ifdef __linux__
#include <sys/prctl.h>
#endif

#include <obs.h>
#include <obs-module.h>

/* Never dereferenced: only passed back to free_type_data. */
#define TYPE_DATA ((void *)(uintptr_t)UINT64_C(0xA1B2C3D4E5F60718))

static int frees;
static void *freed;

static void free_type_data(void *type_data)
{
	frees++;
	freed = type_data;
}

enum kind { KIND_SOURCE, KIND_OUTPUT, KIND_ENCODER, KIND_SERVICE };

enum cut { CUT_IN_TYPE_DATA, CUT_IN_FREE_HALF, CUT_IN_FREE_SHORT, CUT_AT_FREE_END, CUT_SIZEOF };

struct reg_case {
	enum kind kind;
	enum cut cut;
	const char *id;
};

struct result {
	int frees;
	void *freed;
	bool registered;
};

/* Room after the structure, so a size never reads past the caller's
 * buffer. */
union info {
	struct obs_source_info source;
	struct obs_output_info output;
	struct obs_encoder_info encoder;
	struct obs_service_info service;
	unsigned char bytes[2048];
};

#define LAYOUT(type)                                    \
	do {                                            \
		*type_data = offsetof(type, type_data); \
		*free_fn = offsetof(type, free_type_data); \
		*size = sizeof(type);                   \
	} while (false)

static void layout(enum kind kind, size_t *type_data, size_t *free_fn, size_t *size)
{
	switch (kind) {
	case KIND_SOURCE:
		LAYOUT(struct obs_source_info);
		return;
	case KIND_OUTPUT:
		LAYOUT(struct obs_output_info);
		return;
	case KIND_ENCODER:
		LAYOUT(struct obs_encoder_info);
		return;
	case KIND_SERVICE:
		LAYOUT(struct obs_service_info);
		return;
	}
	fail();
}

static size_t size_of_cut(enum kind kind, enum cut cut)
{
	size_t td, fn, whole;
	layout(kind, &td, &fn, &whole);
	/* type_data comes right before free_type_data in all four. */
	assert_int_equal(td + sizeof(void *), fn);
	switch (cut) {
	case CUT_IN_TYPE_DATA:
		return td + sizeof(void *) / 2;
	case CUT_IN_FREE_HALF:
		return fn + sizeof(void (*)(void *)) / 2;
	case CUT_IN_FREE_SHORT:
		return fn + sizeof(void (*)(void *)) - 1;
	case CUT_AT_FREE_END:
		return fn + sizeof(void (*)(void *));
	case CUT_SIZEOF:
		return whole;
	}
	fail();
	return 0;
}

/* free_type_data is called only when both fields lie wholly within size. */
static bool expect_freed(enum cut cut)
{
	return cut == CUT_AT_FREE_END || cut == CUT_SIZEOF;
}

/* A type with type_data and free_type_data and no get_name, so it is
 * refused, registered with `size`. */
static struct result reg(const struct reg_case *c, size_t size)
{
	union info info;
	memset(&info, 0, sizeof(info));
	frees = 0;
	freed = NULL;

	struct result r = {0};
	switch (c->kind) {
	case KIND_SOURCE:
		info.source.id = c->id;
		info.source.type = OBS_SOURCE_TYPE_INPUT;
		info.source.output_flags = OBS_SOURCE_AUDIO;
		info.source.type_data = TYPE_DATA;
		info.source.free_type_data = free_type_data;
		obs_register_source_s(&info.source, size);
		r.registered = obs_source_get_display_name(c->id) != NULL;
		break;
	case KIND_OUTPUT:
		info.output.id = c->id;
		info.output.type_data = TYPE_DATA;
		info.output.free_type_data = free_type_data;
		obs_register_output_s(&info.output, size);
		r.registered = obs_output_get_display_name(c->id) != NULL;
		break;
	case KIND_ENCODER:
		info.encoder.id = c->id;
		info.encoder.type = OBS_ENCODER_VIDEO;
		info.encoder.type_data = TYPE_DATA;
		info.encoder.free_type_data = free_type_data;
		obs_register_encoder_s(&info.encoder, size);
		r.registered = obs_encoder_get_display_name(c->id) != NULL;
		break;
	case KIND_SERVICE:
		info.service.id = c->id;
		info.service.type_data = TYPE_DATA;
		info.service.free_type_data = free_type_data;
		obs_register_service_s(&info.service, size);
		r.registered = obs_service_get_display_name(c->id) != NULL;
		break;
	}
	r.frees = frees;
	r.freed = freed;
	return r;
}

static bool obs_up;

static void run_case(void **state)
{
	const struct reg_case *c = *state;
	if (!obs_up)
		skip();

	size_t size = size_of_cut(c->kind, c->cut);
	struct result r;

#ifdef _WIN32
	r = reg(c, size);
#else
	int fds[2];
	assert_int_equal(pipe(fds), 0);
	fflush(stdout);
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps SIGSEGV and would resume the test runner in the
		 * child; let a crash kill the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGILL, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
		signal(SIGALRM, SIG_DFL);
#ifdef __linux__
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
#endif
		/* A jump into code that loops ends here. */
		alarm(30);
		close(fds[0]);
		struct result child = reg(c, size);
		ssize_t n = write(fds[1], &child, sizeof(child));
		_exit(n == (ssize_t)sizeof(child) ? 0 : 1);
	}
	close(fds[1]);
	size_t got = 0;
	while (got < sizeof(r)) {
		ssize_t n = read(fds[0], (char *)&r + got, sizeof(r) - got);
		if (n < 0 && errno == EINTR)
			continue;
		if (n <= 0)
			break;
		got += (size_t)n;
	}
	close(fds[0]);

	int status = 0;
	while (waitpid(pid, &status, 0) < 0)
		assert_int_equal(errno, EINTR);
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "%s with size %zu: the registration died with signal %d\n", c->id, size,
			WTERMSIG(status));
		fail();
	}
	assert_true(WIFEXITED(status));
	assert_int_equal(WEXITSTATUS(status), 0);
	assert_int_equal(got, sizeof(r));
#endif

	if (r.frees != (expect_freed(c->cut) ? 1 : 0) || (r.frees && r.freed != TYPE_DATA))
		fprintf(stderr, "%s with size %zu: free_type_data called %d times, last with %p\n", c->id, size,
			r.frees, r.freed);
	assert_false(r.registered);
	assert_int_equal(r.frees, expect_freed(c->cut) ? 1 : 0);
	if (expect_freed(c->cut))
		assert_ptr_equal(r.freed, TYPE_DATA);
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

#define CASES(kind, name)                                                \
	{kind, CUT_IN_TYPE_DATA, "rrtd_" name "_in_type_data"},          \
		{kind, CUT_IN_FREE_HALF, "rrtd_" name "_in_free_half"},  \
		{kind, CUT_IN_FREE_SHORT, "rrtd_" name "_in_free_short"}, \
		{kind, CUT_AT_FREE_END, "rrtd_" name "_at_free_end"},    \
		{kind, CUT_SIZEOF, "rrtd_" name "_sizeof"}

static struct reg_case cases[] = {
	CASES(KIND_SOURCE, "source"),
	CASES(KIND_OUTPUT, "output"),
	CASES(KIND_ENCODER, "encoder"),
	CASES(KIND_SERVICE, "service"),
};

int main(void)
{
	struct CMUnitTest tests[sizeof(cases) / sizeof(cases[0])];
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct CMUnitTest t = cmocka_unit_test_prestate(run_case, &cases[i]);
		t.name = cases[i].id;
		tests[i] = t;
	}
	return cmocka_run_group_tests_name("test_register_refused_type_data", tests, group_setup, group_teardown);
}
