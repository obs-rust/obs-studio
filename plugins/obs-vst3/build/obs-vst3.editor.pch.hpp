#pragma once

#include "plugins/obs-vst3/VST3EditorWindow.h"
#ifdef __linux__
#include "plugins/obs-vst3/VST3HostApp.h"
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

#ifdef __linux__
#include <QGuiApplication>
#include <QTimer>
#endif

#ifdef _WIN32
#include <QByteArray>
#include <QIcon>
#include <QImage>
#include <QPixmap>
#endif

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <memory>
#include <string>

#ifdef _WIN32
#include <windows.h>
#endif
