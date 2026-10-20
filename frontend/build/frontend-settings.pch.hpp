/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.settings. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* settings/OBSBasicSettings.cpp */
#include "frontend/settings/OBSBasicSettings.hpp"
#include "frontend/settings/OBSHotkeyBackgroundBanner.hpp"
#include "frontend/settings/OBSHotkeyLabel.hpp"
#include "frontend/settings/OBSHotkeyWidget.hpp"

#include <components/Multiview.hpp>
#include <components/OBSSourceLabel.hpp>
#include <components/SilentUpdateCheckBox.hpp>
#include <components/SilentUpdateSpinBox.hpp>
#ifdef YOUTUBE_ENABLED
#include <docks/YouTubeAppDock.hpp>
#endif
#include <utility/audio-encoders.hpp>
#include <utility/BaseLexer.hpp>
#include <utility/encoder-fallback.h>
#include <utility/FFmpegCodec.hpp>
#include <utility/FFmpegFormat.hpp>
#include <utility/output-rescale.h>
#include <utility/replay-buffer-estimate.hpp>
#include <utility/SettingsEventFilter.hpp>
#ifdef YOUTUBE_ENABLED
#include <utility/YoutubeApiWrappers.hpp>
#endif
#include <widgets/OBSBasic.hpp>
#include <widgets/OBSProjector.hpp>

#include <properties-view.hpp>
#include <qt-wrappers.hpp>

#include <QCompleter>
#include <QFontMetrics>
#include <QStandardItemModel>

#include <sstream>

/* settings/OBSBasicSettings_A11y.cpp */

#include <QColorDialog>

/* settings/OBSBasicSettings_Appearance.cpp */

#include <OBSApp.hpp>
#include <utility/platform.hpp>

/* settings/OBSBasicSettings_Stream.cpp */

#ifdef YOUTUBE_ENABLED
#include <docks/YouTubeAppDock.hpp>
#endif
#include <oauth/OAuth.hpp>
#ifdef YOUTUBE_ENABLED
#include <utility/YoutubeApiWrappers.hpp>
#endif
#include <utility/GoLiveAPI_Network.hpp>
#include <utility/GoLiveAPI_PostData.hpp>
#include <utility/MultitrackVideoError.hpp>
#include <utility/multitrack-ladder.h>
#include <utility/stream-service-select.h>

#include <QApplication>
#include <QDateTime>
#include <QHeaderView>
#include <QTableWidget>
#include <QUuid>

#include <nlohmann/json.hpp>

#include <climits>
#include <string>
#include <vector>

/* settings/OBSHotkeyBackgroundBanner.cpp */

#include <util/dstr.hpp>

#include <QCheckBox>
#include <QClipboard>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProcess>
#include <QThread>
#include <QAccessible>
#include <QCoreApplication>
#include <QStyle>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>

/* settings/OBSHotkeyEdit.cpp */
#include "frontend/settings/OBSHotkeyEdit.hpp"
#include "frontend/settings/OBSHotkeyCapture.hpp"

#include <QKeyEvent>

/* settings/OBSHotkeyLabel.cpp */

#include <QEnterEvent>
