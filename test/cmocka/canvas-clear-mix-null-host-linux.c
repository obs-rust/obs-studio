/* Isolate the existing NULL-canvas fault before any threads are started. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
#include <obs.h>

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
		/* Characterized, not endorsed: NULL is outside the C valid domain. */
		clear(NULL);
		_exit(0);
	}
	int status = 0;
	assert_int_equal(waitpid(child, &status, 0), child);
	assert_true(WIFSIGNALED(status));
	assert_true(WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS);
}
