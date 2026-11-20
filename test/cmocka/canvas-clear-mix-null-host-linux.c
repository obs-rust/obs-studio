/* Isolate the existing NULL-canvas fault before any threads are started. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
#include <obs.h>

/* Present only when a sanitizer runtime is linked (the asan job builds the
 * whole tree with ASan and UBSan). */
extern void __sanitizer_set_report_fd(void *fd) __attribute__((weak));

void canvas_clear_mix_null_probe(void (*clear)(obs_canvas_t *));

void canvas_clear_mix_null_probe(void (*clear)(obs_canvas_t *))
{
	pid_t child = fork();
	assert_true(child >= 0);
	if (child == 0) {
		const struct rlimit no_core = {0, 0};
		if (setrlimit(RLIMIT_CORE, &no_core) != 0)
			_exit(2);
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		/* The NULL dereference below is the behavior under test, so a UBSan
		 * "member access within null pointer" report for it is expected.
		 * Send this child's reports to /dev/null, or the sanitizer triage
		 * counts the probe as a finding: GCC's UBSan prints to stderr (it
		 * ignores log_path), clang's honors log_path, so cover both. */
		int devnull = open("/dev/null", O_WRONLY);
		if (devnull < 0 || dup2(devnull, STDERR_FILENO) < 0)
			_exit(3);
		if (__sanitizer_set_report_fd)
			__sanitizer_set_report_fd((void *)(intptr_t)devnull);
		/* Characterized, not endorsed: NULL is outside the C valid domain. */
		clear(NULL);
		_exit(0);
	}
	int status = 0;
	assert_int_equal(waitpid(child, &status, 0), child);
	assert_true(WIFSIGNALED(status));
	assert_true(WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS);
}
