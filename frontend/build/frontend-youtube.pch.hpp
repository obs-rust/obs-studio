/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.youtube. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* dialogs/OBSYoutubeActions.cpp */
#include "frontend/dialogs/OBSYoutubeActions.hpp"

#include <docks/YouTubeAppDock.hpp>
#include <utility/youtube-broadcast-settings.h>
#include <widgets/OBSBasic.hpp>

#include <qt-wrappers.hpp>

#include <QDesktopServices>
#include <QFileInfo>
#include <QImageReader>
#include <QStandardPaths>
#include <QToolTip>

/* docks/YouTubeAppDock.cpp */
#include "frontend/docks/YouTubeAppDock.hpp"

#include <utility/YoutubeApiWrappers.hpp>

#include <QUuid>
#include <nlohmann/json.hpp>

/* docks/YouTubeChatDock.cpp */
#include "frontend/docks/YouTubeChatDock.hpp"

#include <QHBoxLayout>
#include <QPushButton>

/* oauth/YoutubeAuth.cpp */
#include "frontend/oauth/YoutubeAuth.hpp"

#ifdef BROWSER_AVAILABLE
#include <docks/YouTubeChatDock.hpp>
#endif
#include <oauth/AuthListener.hpp>
#include <utility/obf.h>

#include <ui-config.h>

#include <QRandomGenerator>

/* utility/YoutubeApiWrappers.cpp */
#include "frontend/utility/YoutubeApiWrappers.hpp"

#include <utility/RemoteTextThread.hpp>

#include <QFile>
#include <QMimeDatabase>

#include "frontend/utility/platform.hpp"
