// SPDX-License-Identifier: GPL-3.0-or-later
#include "plugins/obs-vst3/build/obs-vst3.linux.pch.hpp"

namespace {
#ifdef OBS_VST3_RUST_HOST
std::unique_ptr<QtRunLoop> vst3LinuxRunLoop;
#else
std::unique_ptr<RunLoopImpl> vst3LinuxRunLoop;
#endif
} // namespace

VST3Backend vst3_host_backend()
{
	switch (obs_get_nix_platform()) {
	case OBS_NIX_PLATFORM_X11_EGL:
		return VST3Backend::X11;
	case OBS_NIX_PLATFORM_WAYLAND:
		return VST3Backend::Wayland;
	default:
		return VST3Backend::Unknown;
	}
}

void vst3_attach_run_loop(VST3HostApp &host)
{
#ifdef OBS_VST3_RUST_HOST
	vst3LinuxRunLoop = std::make_unique<QtRunLoop>();
	host.setRunLoop(vst3LinuxRunLoop->interface());
#else
	vst3LinuxRunLoop = std::make_unique<RunLoopImpl>();
	host.setRunLoop(vst3LinuxRunLoop.get());
#endif
}

void vst3_detach_run_loop(VST3HostApp &host)
{
	host.setRunLoop(nullptr);
#ifndef OBS_VST3_RUST_HOST
	vst3LinuxRunLoop->stop();
#endif
	vst3LinuxRunLoop.reset();
}

void vst3_prepare_plugin_environment()
{
	// Work around LSP's GLX editor crash with NVIDIA by selecting Cairo.
	setenv("LSP_WS_LIB_GLXSURFACE", "off", 1);
	blog(LOG_INFO, "[VST3 Plugin:] Forcing Cairo for LSP editors");
}

const char *vst3_scanner_executable()
{
#ifndef OBS_VST3_SCANNER_NAME
#define OBS_VST3_SCANNER_NAME "obs-vst3-scanner"
#endif
	return OBS_VST3_SCANNER_NAME;
}

#undef OBS_VST3_SCANNER_NAME
