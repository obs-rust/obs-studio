/* Precompiled header (BUILD.md) for the libobs unity groups libobs.ported-posix. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* util/pipe-posix.c */
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>
#include <errno.h>
#include <spawn.h>
#include <fcntl.h>
#include <dirent.h>
#include <limits.h>
#include <stdlib.h>
#include <sys/resource.h>

#include "util/bmem.h"
#include "util/pipe.h"

/* util/platform-nix.c */
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <dirent.h>
#include <stdlib.h>
#include <limits.h>
#include <dlfcn.h>
#include <glob.h>
#include <time.h>
#include <signal.h>
#include <uuid/uuid.h>

#include "obsconfig.h"

#if !defined(__APPLE__)
#include <sys/times.h>
#include <sys/wait.h>
#include <libgen.h>
#if defined(__FreeBSD__) || defined(__OpenBSD__)
#include <sys/param.h>
#include <sys/queue.h>
#include <sys/socket.h>
#include <sys/sysctl.h>
#include <sys/user.h>
#include <unistd.h>
#if defined(__FreeBSD__)
#include <libprocstat.h>
#endif
#else
#include <sys/resource.h>
#endif
#if !defined(__OpenBSD__)
#include <sys/sysinfo.h>
#endif
#include <spawn.h>
#endif

#include "util/darray.h"
#include "util/dstr.h"
#include "util/platform.h"
#include "util/threading.h"

/* util/platform-file-host-posix.c, util/profiler-io-host-posix.c */
#include <zlib.h>

#include "util/bmem.h"
#include "util/profiler-io-host.h"
