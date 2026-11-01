/*
 * Copyright (c) 2023 Lain Bailey <lain@obsproject.com>
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */

#include "build/libobs-ported-posix.pch.h"

extern char **environ;

struct os_process_pipe {
	bool read_pipe;
	int pid;
	FILE *file;
	FILE *err_file;
};

/* posix_spawn_file_actions_addclosefrom_np */
#if (defined(__GLIBC__) && (__GLIBC__ > 2 || (__GLIBC__ == 2 && __GLIBC_MINOR__ >= 34))) || \
	(defined(__FreeBSD__) && __FreeBSD_version >= 1301000)
#define HAVE_SPAWN_CLOSEFROM 1
#endif

#if !defined(__APPLE__) && !defined(HAVE_SPAWN_CLOSEFROM)
/* A close action for fd if the child would inherit it. */
static bool close_if_inherited(posix_spawn_file_actions_t *file_actions, int fd)
{
	int flags = fcntl(fd, F_GETFD);
	return flags == -1 || (flags & FD_CLOEXEC) || posix_spawn_file_actions_addclose(file_actions, fd) == 0;
}

/* Without a closefrom action, close each descriptor above stderr that is
 * open now and lacks close-on-exec. One closed in between makes the child's
 * close fail with EBADF, which posix_spawn ignores. */
static bool close_inherited_fds(posix_spawn_file_actions_t *file_actions)
{
	DIR *dir = opendir("/proc/self/fd");
	if (dir) {
		int self = dirfd(dir);
		bool ok = true;
		struct dirent *ent;
		while (ok && (ent = readdir(dir)) != NULL) {
			char *end;
			long fd = strtol(ent->d_name, &end, 10);
			if (end == ent->d_name || *end || fd <= STDERR_FILENO || fd > INT_MAX || fd == self)
				continue;
			ok = close_if_inherited(file_actions, (int)fd);
		}
		closedir(dir);
		return ok;
	}

	/* No /proc: every open descriptor is below the soft limit, which the
	 * kernel checks when it hands one out. */
	struct rlimit lim;
	long max = sysconf(_SC_OPEN_MAX);
	if (getrlimit(RLIMIT_NOFILE, &lim) == 0 && lim.rlim_cur != RLIM_INFINITY && (rlim_t)max < lim.rlim_cur)
		max = lim.rlim_cur > INT_MAX ? INT_MAX : (long)lim.rlim_cur;
	if (max <= 0)
		max = 1024;
	for (int fd = STDERR_FILENO + 1; fd < max; fd++) {
		if (!close_if_inherited(file_actions, fd))
			return false;
	}
	return true;
}
#endif

/* Closes every descriptor above stderr in the child. */
static bool limit_inherited_fds(posix_spawn_file_actions_t *file_actions, posix_spawnattr_t *attr, bool read_pipe)
{
#if defined(__APPLE__)
	/* Only the descriptors the file actions name are inherited. stdin (read
	 * mode) or stdout (write mode) is not replaced, so name it. */
	int kept = read_pipe ? STDIN_FILENO : STDOUT_FILENO;
	if (posix_spawnattr_init(attr) != 0)
		return false;
	if (posix_spawnattr_setflags(attr, POSIX_SPAWN_CLOEXEC_DEFAULT) != 0 ||
	    (fcntl(kept, F_GETFD) != -1 && posix_spawn_file_actions_addinherit_np(file_actions, kept) != 0)) {
		posix_spawnattr_destroy(attr);
		return false;
	}
	return true;
#elif defined(HAVE_SPAWN_CLOSEFROM)
	UNUSED_PARAMETER(attr);
	UNUSED_PARAMETER(read_pipe);
	return posix_spawn_file_actions_addclosefrom_np(file_actions, STDERR_FILENO + 1) == 0;
#else
	UNUSED_PARAMETER(attr);
	UNUSED_PARAMETER(read_pipe);
	return close_inherited_fds(file_actions);
#endif
}

static os_process_pipe_t *os_process_pipe_create_internal(const char *bin, char **argv, const char *type)
{
	struct os_process_pipe process_pipe = {0};
	struct os_process_pipe *out;
	posix_spawn_file_actions_t file_actions;

	if (!bin || !argv || !type) {
		return NULL;
	}

	process_pipe.read_pipe = *type == 'r';

	int mainfds[2] = {0};
	int errfds[2] = {0};

	if (pipe(mainfds) != 0) {
		return NULL;
	}

	if (pipe(errfds) != 0) {
		close(mainfds[0]);
		close(mainfds[1]);

		return NULL;
	}

	if (posix_spawn_file_actions_init(&file_actions) != 0) {
		close(mainfds[0]);
		close(mainfds[1]);
		close(errfds[0]);
		close(errfds[1]);

		return NULL;
	}

	fcntl(mainfds[0], F_SETFD, FD_CLOEXEC);
	fcntl(mainfds[1], F_SETFD, FD_CLOEXEC);
	fcntl(errfds[0], F_SETFD, FD_CLOEXEC);
	fcntl(errfds[1], F_SETFD, FD_CLOEXEC);

	if (process_pipe.read_pipe) {
		posix_spawn_file_actions_addclose(&file_actions, mainfds[0]);
		if (mainfds[1] != STDOUT_FILENO) {
			posix_spawn_file_actions_adddup2(&file_actions, mainfds[1], STDOUT_FILENO);
			posix_spawn_file_actions_addclose(&file_actions, mainfds[1]);
		}
	} else {
		posix_spawn_file_actions_addclose(&file_actions, mainfds[1]);
		if (mainfds[0] != STDIN_FILENO) {
			posix_spawn_file_actions_adddup2(&file_actions, mainfds[0], STDIN_FILENO);
			posix_spawn_file_actions_addclose(&file_actions, mainfds[0]);
		}
	}

	posix_spawn_file_actions_addclose(&file_actions, errfds[0]);
	if (errfds[1] != STDERR_FILENO) {
		posix_spawn_file_actions_adddup2(&file_actions, errfds[1], STDERR_FILENO);
		posix_spawn_file_actions_addclose(&file_actions, errfds[1]);
	}

	/* The child keeps its standard streams and nothing else. Every other
	 * descriptor without close-on-exec (sockets, files, the log) would stay
	 * open in it as long as it runs: a socket the parent closed then stays
	 * bound, e.g. an SRT Media Source's port during a recording
	 * (obsproject/obs-studio#4596). */
	posix_spawnattr_t attr;
	posix_spawnattr_t *attrp = NULL;
	if (!limit_inherited_fds(&file_actions, &attr, process_pipe.read_pipe)) {
		posix_spawn_file_actions_destroy(&file_actions);
		close(mainfds[0]);
		close(mainfds[1]);
		close(errfds[0]);
		close(errfds[1]);

		return NULL;
	}
#ifdef __APPLE__
	attrp = &attr;
#endif

	int pid;
	int ret = posix_spawn(&pid, bin, &file_actions, attrp, (char *const *)argv, environ);

	posix_spawn_file_actions_destroy(&file_actions);
	if (attrp)
		posix_spawnattr_destroy(attrp);

	if (ret != 0) {
		close(mainfds[0]);
		close(mainfds[1]);
		close(errfds[0]);
		close(errfds[1]);

		return NULL;
	}

	close(errfds[1]);
	process_pipe.err_file = fdopen(errfds[0], "r");

	if (process_pipe.read_pipe) {
		close(mainfds[1]);
		process_pipe.file = fdopen(mainfds[0], "r");
	} else {
		close(mainfds[0]);
		process_pipe.file = fdopen(mainfds[1], "w");
	}

	process_pipe.pid = pid;

	out = bmalloc(sizeof(os_process_pipe_t));
	*out = process_pipe;
	return out;
}

os_process_pipe_t *os_process_pipe_create(const char *cmd_line, const char *type)
{
	if (!cmd_line)
		return NULL;

	/* posix_spawn takes char *const argv[] but does not modify it. */
	char *argv[4] = {"sh", "-c", (char *)(uintptr_t)cmd_line, NULL};
	return os_process_pipe_create_internal("/bin/sh", argv, type);
}

os_process_pipe_t *os_process_pipe_create2(const os_process_args_t *args, const char *type)
{
	char **argv = os_process_args_get_argv(args);
	return os_process_pipe_create_internal(argv[0], argv, type);
}

int os_process_pipe_destroy(os_process_pipe_t *pp)
{
	int ret = 0;

	if (pp) {
		int status;

		fclose(pp->file);
		pp->file = NULL;

		fclose(pp->err_file);
		pp->err_file = NULL;

		do {
			ret = waitpid(pp->pid, &status, 0);
		} while (ret == -1 && errno == EINTR);

		if (ret != -1 && WIFEXITED(status))
			ret = (int)(char)WEXITSTATUS(status);
		bfree(pp);
	}

	return ret;
}

size_t os_process_pipe_read(os_process_pipe_t *pp, uint8_t *data, size_t len)
{
	if (!pp) {
		return 0;
	}
	if (!pp->read_pipe) {
		return 0;
	}

	return fread(data, 1, len, pp->file);
}

size_t os_process_pipe_read_err(os_process_pipe_t *pp, uint8_t *data, size_t len)
{
	if (!pp) {
		return 0;
	}

	return fread(data, 1, len, pp->err_file);
}

size_t os_process_pipe_write(os_process_pipe_t *pp, const uint8_t *data, size_t len)
{
	if (!pp) {
		return 0;
	}
	if (pp->read_pipe) {
		return 0;
	}
	if (ferror(pp->file)) {
		return 0;
	}

	/* A short fwrite reports an error; retrying can count undelivered data. */
	return fwrite(data, 1, len, pp->file);
}

#undef HAVE_SPAWN_CLOSEFROM
