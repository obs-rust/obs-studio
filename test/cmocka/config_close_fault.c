#include <assert.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

/* Single-threaded fault injection, confined to the armed fixture's inode. */
static char armed_path[4096];
static int faults;
static int close_attempts;
static int inside_fclose;
static int spare_fd = -1;
static int reused_fd = -1;

void config_close_fault_arm(const char *path)
{
	assert(strlen(path) < sizeof(armed_path));
	strcpy(armed_path, path);
	faults = 0;
	close_attempts = 0;
	spare_fd = open("/dev/null", O_RDONLY);
	if (spare_fd < 0)
		abort();
}

int config_close_fault_attempts(void)
{
	return close_attempts;
}

int config_close_fault_disarm(void)
{
	armed_path[0] = 0;
	int fd = reused_fd;
	reused_fd = -1;
	if (fd >= 0)
		close(fd);
	close(spare_fd);
	spare_fd = -1;
	return faults;
}

static void reuse_closed_fd(int fd)
{
	/* Keep this descriptor live: a retry would close an unrelated resource. */
	if (dup2(spare_fd, fd) != fd)
		abort();
	reused_fd = fd;
}

static int matches(int fd)
{
	struct stat a, b;
	return armed_path[0] && !faults && fstat(fd, &a) == 0 && stat(armed_path, &b) == 0 && a.st_dev == b.st_dev &&
	       a.st_ino == b.st_ino;
}

#ifndef __APPLE__
#define fault_close close
#define fault_fclose fclose
#endif

int fault_close(int fd)
{
#ifdef __APPLE__
	/* dyld leaves calls from the interposing image bound to the original. */
	static int (*real_close)(int) = close;
#else
	static int (*real_close)(int);
	if (!real_close)
		real_close = (int (*)(int))dlsym(RTLD_NEXT, "close");
	if (!real_close)
		abort();
#endif
	int inject = !inside_fclose && matches(fd);
	if (!inside_fclose && (inject || fd == reused_fd))
		++close_attempts;
	int ret = real_close(fd);
	if (inject && ret == 0) {
		reuse_closed_fd(fd);
		++faults;
		errno = EIO;
		return -1;
	}
	return ret;
}

int fault_fclose(FILE *f)
{
#ifdef __APPLE__
	static int (*real_fclose)(FILE *) = fclose;
#else
	static int (*real_fclose)(FILE *);
	if (!real_fclose)
		real_fclose = (int (*)(FILE *))dlsym(RTLD_NEXT, "fclose");
	if (!real_fclose)
		abort();
#endif
	int fd = fileno(f);
	int inject = matches(fd);
	if (inject || fd == reused_fd)
		++close_attempts;
	++inside_fclose;
	int ret = real_fclose(f);
	--inside_fclose;
	if (inject && ret == 0) {
		reuse_closed_fd(fd);
		++faults;
		errno = EIO;
		return EOF;
	}
	return ret;
}

#ifdef __APPLE__
/* dyld interposes calls from libobs as well as from the test executable. */
__attribute__((used, section("__DATA,__interpose"))) static const struct {
	const void *replacement;
	const void *original;
} interposers[] = {{(const void *)fault_close, (const void *)close},
		   {(const void *)fault_fclose, (const void *)fclose}};
#endif
