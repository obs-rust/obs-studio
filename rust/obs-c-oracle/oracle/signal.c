/* Test-only oracle: the original libobs/callback/signal.c.h, unmodified,
 * with its exports renamed to oracle_* so it can link next to the Rust
 * implementation. It parses declarations with the decl oracle
 * (oracle/decl.c), so the whole handler is the original C. bmalloc, blog
 * and the calldata functions stay the real symbols: the Rust port and this
 * oracle share them.
 *
 * -std=c11 hides the X/Open PTHREAD_MUTEX_RECURSIVE that
 * pthread_mutex_init_recursive in util/threading.h uses; ask for it. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>

#define signal_handler_create oracle_signal_handler_create
#define signal_handler_destroy oracle_signal_handler_destroy
#define signal_handler_add oracle_signal_handler_add
#define signal_handler_connect oracle_signal_handler_connect
#define signal_handler_connect_ref oracle_signal_handler_connect_ref
#define signal_handler_disconnect oracle_signal_handler_disconnect
#define signal_handler_connect_global oracle_signal_handler_connect_global
#define signal_handler_disconnect_global oracle_signal_handler_disconnect_global
#define signal_handler_remove_current oracle_signal_handler_remove_current
#define signal_handler_signal oracle_signal_handler_signal

#define parse_decl_string oracle_parse_decl_string

/* Index loops compare size_t with long and store pointer differences.
 * The oracle builds with -Wsign-conversion and libobs does not. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif

#include "callback/signal.c.h"
