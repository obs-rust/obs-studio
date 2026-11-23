// SPDX-License-Identifier: GPL-3.0-or-later
#include "plugins/obs-vst3/build/obs-vst3.host.windows.pch.hpp"

VST3Backend vst3_host_backend()
{
	return VST3Backend::Windows;
}

void vst3_attach_run_loop(VST3HostApp &) {}
void vst3_detach_run_loop(VST3HostApp &) {}
void vst3_prepare_plugin_environment() {}

const char *vst3_scanner_executable()
{
	return "obs-vst3-scanner.exe";
}
