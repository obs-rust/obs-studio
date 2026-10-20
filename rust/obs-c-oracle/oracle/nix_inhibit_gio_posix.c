/* Test-only oracle: the original libobs/util/platform-nix-inhibit.c,
 * unmodified, as built with GIO_FOUND, with its exports renamed to
 * oracle_gio_*. The portal and D-Bus inhibitors it calls are the counted
 * stand-ins in nix_inhibit_stubs_posix.c. Linux and FreeBSD only. */
#if defined(__linux__) || defined(__FreeBSD__)
#include "nix_inhibit_hooks.h"

#define GIO_FOUND
#define os_inhibit_sleep_create oracle_gio_os_inhibit_sleep_create
#define os_inhibit_sleep_set_active oracle_gio_os_inhibit_sleep_set_active
#define os_inhibit_sleep_destroy oracle_gio_os_inhibit_sleep_destroy

#include "util/platform-nix-inhibit.c.h"
#else
typedef int oracle_nix_inhibit_gio_unused;
#endif
