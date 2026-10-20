/* Test-only: included by the nix_inhibit*_posix.c oracles before the
 * original libobs/util/platform-nix-inhibit.c, so that what it calls outside
 * itself goes to the counted stand-ins in nix_inhibit_stubs_posix.c. Defined
 * before any header, so the system and libobs prototypes take the new names
 * too. The portal and D-Bus inhibitors keep their real names: the stand-ins
 * define them, because the Rust shim built with the gio feature calls them
 * by those names. */
#pragma once

/* The Flatpak check, access("/.flatpak-info", F_OK). */
#define access oracle_stub_inhibit_access
/* The reset. The stand-in never starts anything and returns an errno. */
#define posix_spawnp oracle_stub_inhibit_posix_spawnp
#define blog oracle_stub_inhibit_blog
/* The reset thread's wait: a few timeouts on request, then the real wait
 * (oracle/threading_posix.c) for the stop event. */
#undef os_event_timedwait
#define os_event_timedwait oracle_stub_inhibit_timedwait
