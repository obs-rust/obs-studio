#include "libobs/build/libobs-nix.pch.h"

/* Rust is always built in release mode (cmake/rust.cmake). Keep this check
 * in the C build so CMake Debug still rejects INVALID, exactly as before. */
void obs_set_nix_platform(enum obs_nix_platform_type platform)
{
	assert(platform != OBS_NIX_PLATFORM_INVALID);
	obs_nix_platform_set(platform);
}
