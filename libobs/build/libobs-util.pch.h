/* Precompiled header (BUILD.md) for the libobs unity groups libobs.ported-util, libobs.ported-util-io, libobs.rust-variadic, libobs.util-cf, libobs.util-libc:
 * util/ alone, without the core, so the C oracle and C tests can compile them. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* util/array-serializer.c */
#include "util/darray.h"
#include "util/array-serializer.h"

/* util/base.c */
#include <stdio.h>
#include <stdlib.h>

#include "util/c99defs.h"
#include "util/base.h"
#include "util/threading.h"

/* util/bitstream.c */
#include "util/bitstream.h"

#include <string.h>

/* util/bmem.c */
#include "util/bmem.h"
#include "util/platform.h"

/* util/buffered-file-serializer.c */
#include "util/buffered-file-serializer.h"

#include <inttypes.h>

#include "util/deque.h"
#include "util/dstr.h"

/* util/cf-tokenizer.c */
#include <ctype.h>
#include "util/cf-lexer.h"

/* util/config-file.c */
#include <wchar.h>

#include "util/config-file.h"
#include "util/lexer.h"
#include "util/uthash.h"

/* util/crc32.c */
#include "util/crc32.h"

/* util/dstr.c */
#include <stddef.h>
#include <assert.h>
#include <wctype.h>
#include <limits.h>

#include "util/utf8.h"

/* util/file-serializer.c */
#include "util/file-serializer.h"

/* util/pipe.c */
#include "util/pipe.h"

/* util/platform-number.c */
#include <locale.h>

/* util/profiler-snapshot.c */
#include "util/profiler-snapshot.h"

/* util/profiler.c */
#include "util/profiler.h"

/* util/task.c */
#include "util/task.h"

/* util/text-lookup.c */

#include "util/text-lookup.h"

/* util/pipe-variadic.c */
#include <stdarg.h>

/* util/utf8.c */
#include <wchar.h>
#include "util/utf8.h"
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#include "util/c99defs.h"
#endif

/* util/cf-lexer.c */
#include <stdio.h>

/* util/cf-parser.c */
#include "util/cf-parser.h"

/* util/dstr-libc.c */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include "util/c99defs.h"
#include "util/dstr.h"
#include "util/bmem.h"
