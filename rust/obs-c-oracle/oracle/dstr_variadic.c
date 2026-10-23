/* Test-only oracle: libobs/util/dstr-variadic.c, the C copy of
 * dstr-libc.c's printf family that the hybrid libobs compiles (stable Rust
 * cannot define a C variadic function), under variadic_* names. Tier 3
 * compares it with the original in oracle/dstr_libc.c (oracle_dstr_printf,
 * ...). It calls only the dstr.h inline helpers. */

#define dstr_printf variadic_dstr_printf
#define dstr_catf variadic_dstr_catf
#define dstr_vprintf variadic_dstr_vprintf
#define dstr_vcatf variadic_dstr_vcatf

#include "util/dstr-variadic.c.h"
