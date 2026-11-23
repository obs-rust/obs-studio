#pragma once

#include "plugins/obs-vst3/VST3Scanner.h"
#include "plugins/obs-vst3/vst3-discovery-platform.h"

#include <pluginterfaces/base/ipluginbase.h>
#include <public.sdk/source/vst/hosting/module.h>
#include <public.sdk/source/vst/moduleinfo/moduleinfo.h>
#include <public.sdk/source/vst/moduleinfo/moduleinfoparser.h>

#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

#include <algorithm>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>
