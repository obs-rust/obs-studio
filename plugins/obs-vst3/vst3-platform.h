// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include "plugins/obs-vst3/VST3HostApp.h"

VST3Backend vst3_host_backend();
void vst3_attach_run_loop(VST3HostApp &host);
void vst3_detach_run_loop(VST3HostApp &host);
void vst3_prepare_plugin_environment();
const char *vst3_scanner_executable();
Steinberg::FIDString vst3_editor_platform_type(VST3Backend backend);
