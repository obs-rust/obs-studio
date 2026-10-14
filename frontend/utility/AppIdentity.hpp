#pragma once

#include <string>
#include <string_view>

#ifdef OBS_RUST_VERSION
#define OBS_CONFIG_DIRECTORY "obs-studio-rust"
#define OBS_DESKTOP_ID "io.github.obs_rust.OBSStudioRust"
#define OBS_RUN_ONCE_NAME "OBSStudioRustCore"
#define OBS_PORTABLE_RUN_ONCE_NAME "OBSStudioRustPortable"
#define OBS_INSTANCE_SOCKET_PREFIX "/io/github/obs_rust/OBSStudioRust"
#define OBS_WINDOWS_APP_ID L"io.github.obs_rust.OBSStudioRust"
#define OBS_BSD_THREAD_NAME "OBS Rust runonce"
#else
#define OBS_CONFIG_DIRECTORY "obs-studio"
#define OBS_DESKTOP_ID "com.obsproject.Studio"
#define OBS_RUN_ONCE_NAME "OBSStudioCore"
#define OBS_PORTABLE_RUN_ONCE_NAME "OBSStudioPortable"
#define OBS_INSTANCE_SOCKET_PREFIX "/com/obsproject"
#define OBS_BSD_THREAD_NAME "OBS runonce"
#endif

// Callers use the historical relative names; the frontend owns their identity.
// The generic libobs config-path API keeps its existing behavior.
inline std::string AppConfigRelativePath(const char *name)
{
	if (!name) {
		return {};
	}
	std::string_view path(name);
	constexpr std::string_view upstream = "obs-studio";
	if (path == upstream || (path.size() > upstream.size() && path.substr(0, upstream.size()) == upstream &&
				 (path[upstream.size()] == '/' || path[upstream.size()] == '\\'))) {
		return OBS_CONFIG_DIRECTORY + std::string(path.substr(upstream.size()));
	}
	return std::string(path);
}
