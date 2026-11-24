#pragma once

#include "plugins/obs-vst3/VST3EditorWindow.h"

#ifdef OBS_VST3_RUST_HOST
#include "plugins/obs-vst3/vst3-rust-host.h"
#endif

#include <pluginterfaces/gui/iplugview.h>
#include <pluginterfaces/gui/iplugviewcontentscalesupport.h>

#include <obs-frontend-api.h>
#include <QCloseEvent>
#include <QEvent>
#include <QMainWindow>
#include <QResizeEvent>
#include <QString>
#include <QWindow>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <memory>
#include <string>
#include <utility>
