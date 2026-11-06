/* Precompiled header (BUILD.md) for the libobs unity groups libobs.ported-windows-platform. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* util/platform-windows.c */
#include <windows.h>
#include <mmsystem.h>
#include <shellapi.h>
#include <shlobj.h>
#include <intrin.h>
#include <psapi.h>
#include <math.h>
#include <rpc.h>

#include "util/base.h"
#include "util/platform.h"
#include "util/darray.h"
#include "util/dstr.h"
#include "util/util_uint64.h"
#include "util/windows/win-registry.h"
#include "util/windows/win-version.h"

#include <pthread.h>

/* util/platform-file-host-windows.c, util/profiler-io-host-windows.c */
#include <stdio.h>
#include <sys/stat.h>
#include <zlib.h>

#include "util/bmem.h"
#include "util/profiler-io-host.h"
