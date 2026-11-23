#pragma once

#include "plugins/obs-vst3/RunLoopImpl.h"
#include "plugins/obs-vst3/vst3-platform.h"

#include <obs-nix-platform.h>
#include <pluginterfaces/gui/iplugview.h>
#include <util/base.h>

#include <QObject>
#include <QSocketNotifier>
#include <QTimer>
#include <algorithm>

#include <cstddef>
#include <cstdint>
#include <mutex>
#include <vector>
#include <cstdlib>
#include <memory>
