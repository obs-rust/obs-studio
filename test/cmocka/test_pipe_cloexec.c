/* SPDX-License-Identifier: GPL-2.0-or-later */

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <util/pipe.h>

#include <errno.h>
#include <fcntl.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;
int __real_pipe(int fds[2]);
int __real_pipe2(int fds[2], int flags);
int __wrap_pipe(int fds[2]);
int __wrap_pipe2(int fds[2], int flags);

static unsigned probes;
static unsigned inherited;

/* Execute another child before OBS can set FD_CLOEXEC. Both syscalls are
 * wrapped so the test also catches pipe2 without O_CLOEXEC. */
static int probe_pipe(int ret, const int fds[2])
{
	if (ret != 0)
		return ret;

	char read_fd[32], write_fd[32];
	snprintf(read_fd, sizeof(read_fd), "%d", fds[0]);
	snprintf(write_fd, sizeof(write_fd), "%d", fds[1]);
	char *argv[] = {"test_pipe_cloexec", "--probe", read_fd, write_fd, NULL};
	pid_t pid;
	ret = posix_spawn(&pid, "/proc/self/exe", NULL, NULL, argv, environ);
	if (ret != 0) {
		fprintf(stderr, "probe spawn: %s\n", strerror(ret));
		exit(2);
	}

	int status;
	pid_t waited;
	do {
		waited = waitpid(pid, &status, 0);
	} while (waited == -1 && errno == EINTR);
	if (waited != pid || !WIFEXITED(status) || WEXITSTATUS(status) > 3) {
		fprintf(stderr, "exec probe failed\n");
		exit(2);
	}

	unsigned mask = (unsigned)WEXITSTATUS(status);
	inherited += (mask & 1) + ((mask >> 1) & 1);
	probes++;
	return 0;
}

int __wrap_pipe(int fds[2])
{
	return probe_pipe(__real_pipe(fds), fds);
}

int __wrap_pipe2(int fds[2], int flags)
{
	return probe_pipe(__real_pipe2(fds, flags), fds);
}

static bool test_pipe(bool read_pipe)
{
	unsigned before = inherited;
	os_process_pipe_t *pp = os_process_pipe_create(read_pipe ? "printf output; printf diagnostic >&2; exit 7"
								 : "IFS= read -r line && test \"$line\" = input",
						       read_pipe ? "r" : "w");
	if (!pp)
		return false;

	bool io_ok;
	if (read_pipe) {
		uint8_t output[32] = {0}, error[32] = {0};
		size_t n = os_process_pipe_read(pp, output, sizeof(output));
		size_t e = os_process_pipe_read_err(pp, error, sizeof(error));
		io_ok = n == 6 && e == 10 && memcmp(output, "output", 6) == 0 && memcmp(error, "diagnostic", 10) == 0;
	} else {
		const uint8_t input[] = "input\n";
		io_ok = os_process_pipe_write(pp, input, sizeof(input) - 1) == sizeof(input) - 1;
	}
	int status = os_process_pipe_destroy(pp);
	fprintf(stderr, "%s: %u inherited pipe ends\n", read_pipe ? "read" : "write", inherited - before);
	return io_ok && status == (read_pipe ? 7 : 0) && inherited == before;
}

int main(int argc, char **argv)
{
	if (argc == 4 && strcmp(argv[1], "--probe") == 0) {
		int mask = 0;
		for (int i = 0; i < 2; i++) {
			errno = 0;
			if (fcntl(atoi(argv[i + 2]), F_GETFD) >= 0)
				mask |= 1 << i;
			else if (errno != EBADF)
				return 4;
		}
		return mask;
	}

	bool read_ok = test_pipe(true);
	bool write_ok = test_pipe(false);
	fprintf(stderr, "2 tests; %u failed; %u exec probes\n", (unsigned)!read_ok + (unsigned)!write_ok, probes);
	return read_ok && write_ok && probes == 4 ? 0 : 1;
}
