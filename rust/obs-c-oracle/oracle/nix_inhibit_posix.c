/* Test-only oracle: the original libobs/util/platform-nix-inhibit.c,
 * unmodified, as built without GIO (no portal or D-Bus branch), with its
 * exports renamed to oracle_* so it can link next to the Rust shim. Linux and
 * FreeBSD only; nix_inhibit_gio_posix.c is the GIO build. */
#if defined(__linux__) || defined(__FreeBSD__)
#include "nix_inhibit_hooks.h"

#define os_inhibit_sleep_create oracle_os_inhibit_sleep_create
#define os_inhibit_sleep_set_active oracle_os_inhibit_sleep_set_active
#define os_inhibit_sleep_destroy oracle_os_inhibit_sleep_destroy

#include "util/platform-nix-inhibit.c.h"
#else
typedef int oracle_nix_inhibit_unused;
#endif
