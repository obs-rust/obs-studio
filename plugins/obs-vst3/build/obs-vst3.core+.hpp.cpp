#include "plugins/obs-vst3/build/obs-vst3.core.pch.hpp"

#include "plugins/obs-vst3/VST3HostApp.cpp.hpp"
#include "plugins/obs-vst3/VST3Plugin-editor.cpp.hpp"
#ifdef OBS_VST3_RUST_HOST
#include "plugins/obs-vst3/VST3Plugin-rust.cpp.hpp"
#else
#include "plugins/obs-vst3/VST3ComponentHolder.cpp.hpp"
#include "plugins/obs-vst3/VST3Plugin.cpp.hpp"
#endif
#include "plugins/obs-vst3/obs-vst3.cpp.hpp"
#include "plugins/obs-vst3/plugin-main.cpp.hpp"
