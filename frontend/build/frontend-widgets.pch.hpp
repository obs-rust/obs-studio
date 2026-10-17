/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.widgets, frontend.widgets-2, frontend.widgets-3, frontend.widgets-4. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* widgets/AudioMixer.cpp */
#include "frontend/widgets/AudioMixer.hpp"

#include <components/MenuCheckBox.hpp>
#include <dialogs/NameDialog.hpp>
#include <utility/item-widget-helpers.hpp>
#include <utility/preview-audio-sources.hpp>
#include <widgets/OBSBasic.hpp>

#include <Idian/Utils.hpp>

#include <QAction>
#include <QCheckBox>
#include <QHBoxLayout>
#include <QMenu>
#include <QPointer>
#include <QPushButton>
#include <QScrollArea>
#include <QStackedWidget>
#include <QTimer>
#include <QToolBar>
#include <QVBoxLayout>
#include <QWidgetAction>

/* widgets/ColorSelect.cpp */
#include "frontend/widgets/ColorSelect.hpp"

/* widgets/OBSBasic.cpp */
#include "frontend/widgets/OBSBasic.hpp"
#include "frontend/widgets/RecordingShutdown.hpp"
#include "frontend/widgets/RecordingShutdownWait.hpp"
#include "frontend/widgets/VirtualCamShutdown.hpp"
#include "ui-config.h"

#include "frontend/widgets/OBSBasicControls.hpp"
#include "frontend/widgets/OBSBasicStats.hpp"
#include "plugin-manager/PluginManager.hpp"

#include <obs-module.h>

#ifdef YOUTUBE_ENABLED
#include <docks/YouTubeAppDock.hpp>
#endif
#include <dialogs/OBSAbout.hpp>
#include <dialogs/OBSBasicAdvAudio.hpp>
#include <dialogs/OBSBasicFilters.hpp>
#include <dialogs/OBSBasicInteraction.hpp>
#include <dialogs/OBSBasicProperties.hpp>
#include <dialogs/OBSBasicTransform.hpp>
#include <models/SceneCollection.hpp>
#include <settings/OBSBasicSettings.hpp>
#include <utility/QuickTransition.hpp>
#include <utility/SceneRenameDelegate.hpp>
#include <utility/ScreenshotObj.hpp>
#if defined(_WIN32) || defined(WHATSNEW_ENABLED)
#include <utility/WhatsNewInfoThread.hpp>
#endif
#include <utility/GpuStatus.hpp>
#include <utility/profile-activate.h>
#include <widgets/AudioMixer.hpp>
#include <widgets/OBSProjector.hpp>

#include <OBSStudioAPI.hpp>
#ifdef BROWSER_AVAILABLE
#include <browser-panel.hpp>
#endif
#ifdef ENABLE_WAYLAND
#include <obs-nix-platform.h>
#endif
#include <qt-wrappers.hpp>

#include <QActionGroup>
#include <QThread>

#include <mutex>
#include <sstream>
#include <string>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include "Windows.h"
#endif

/* widgets/OBSBasicControls.cpp */
#include "qt-wrappers.hpp"

/* widgets/OBSBasicPreview.cpp */
#include "frontend/widgets/OBSBasicPreview.hpp"

#include <utility/display-helpers.hpp>

/* widgets/OBSBasicStats.cpp */

#include <QGridLayout>
#include <QLabel>

/* widgets/OBSBasicStatusBar.cpp */
#include "frontend/widgets/OBSBasicStatusBar.hpp"
#include "ui_StatusBarWidget.h"

#include <QBoxLayout>
#include <QMessageBox>
#include <QPainter>

/* widgets/OBSBasic_Clipboard.cpp */

#include <utility/filter-clipboard.h>
#include <components/VolumeControl.hpp>
#include <dialogs/OBSBasicSourceSelect.hpp>
#include <utility/transform-clipboard.h>

/* widgets/OBSBasic_ContextToolbar.cpp */

#include <components/ApplicationAudioCaptureToolbar.hpp>
#include <components/AudioCaptureToolbar.hpp>
#include <components/BrowserToolbar.hpp>
#include <components/ColorSourceToolbar.hpp>
#include <components/DeviceCaptureToolbar.hpp>
#include <components/DisplayCaptureToolbar.hpp>
#include <components/GameCaptureToolbar.hpp>
#include <components/ImageSourceToolbar.hpp>
#include <components/MediaControls.hpp>
#include <components/TextSourceToolbar.hpp>
#include <components/WindowCaptureToolbar.hpp>

/* widgets/OBSBasic_Dropfiles.cpp */

#include <QFileInfo>
#include <QMimeData>
#ifdef _WIN32
#include <QSettings>
#endif
#include <QUrlQuery>

/* widgets/OBSBasic_Hotkeys.cpp */

#include <widgets/OBSBasicStats.hpp>

/* widgets/OBSBasic_MainControls.cpp */
#include <utility/AppIdentity.hpp>

#include <dialogs/LogUploadDialog.hpp>
#ifdef ENABLE_IDIAN_PLAYGROUND
#include <dialogs/OBSIdianPlayground.hpp>
#endif
#include <dialogs/OBSLogViewer.hpp>
#ifdef __APPLE__
#include <dialogs/OBSPermissions.hpp>
#endif
#include <dialogs/OBSRemux.hpp>
#ifdef _WIN32
#include <utility/AutoUpdateThread.hpp>
#endif
#include <utility/RemoteTextThread.hpp>
#if defined(_WIN32) || defined(WHATSNEW_ENABLED)
#include <utility/WhatsNewInfoThread.hpp>
#endif
#include <wizards/AutoConfig.hpp>

#include <nlohmann/json.hpp>
#include <QDesktopServices>

/* widgets/OBSBasic_OutputHandler.cpp */

#include <QDir>

/* widgets/OBSBasic_Preview.cpp */

#include <QColorDialog>

/* widgets/OBSBasic_Profiles.cpp */

#ifdef YOUTUBE_ENABLED
#include <docks/YouTubeAppDock.hpp>
#endif
#include <utility/menu-text.h>
#include <utility/profile-remove.h>

#include <util/pipe.h>

#include <QFile>

#include <unordered_set>

/* widgets/OBSBasic_Projectors.cpp */
#include "frontend/widgets/OBSProjector.hpp"

/* widgets/OBSBasic_Recording.cpp */

#include <components/UIValidation.hpp>
#include <utility/output-start-log.hpp>

/* widgets/OBSBasic_SceneCollections.cpp */

#include <dialogs/OBSMissingFiles.hpp>
#include <importer/OBSImporter.hpp>
#include <utility/missing-files-list.h>

#include <filesystem>
#include <vector>

/* widgets/OBSBasic_Scenes.cpp */

#include <utility/scene-list-remove.h>

#include <QLineEdit>

/* widgets/OBSBasic_Streaming.cpp */

#ifdef YOUTUBE_ENABLED
#include <docks/YouTubeAppDock.hpp>
#include <utility/YoutubeApiWrappers.hpp>
#endif

/* widgets/OBSBasic_StudioMode.cpp */

#include <utility/studio-mode-exit.h>

#include <slider-ignorewheel.hpp>

#include <QToolTip>

/* widgets/OBSBasic_Transitions.cpp */

#include <algorithm>

#include <components/MenuButton.hpp>
#include <utility/studio-mode-swap.h>
#include <utility/saved-transition.h>
#include <utility/tbar-position.h>
#include <utility/visibility-transition.h>

/* widgets/OBSBasic_Updater.cpp */

#include <dialogs/OBSWhatsNew.hpp>

#ifdef _WIN32
#include <utility/AutoUpdateThread.hpp>
#endif
#ifdef ENABLE_SPARKLE_UPDATER
#include <utility/MacUpdateThread.hpp>
#include <utility/OBSSparkle.hpp>
#endif
#if defined(_WIN32) || defined(WHATSNEW_ENABLED)
#include <utility/WhatsNewBrowserInitThread.hpp>
#include <utility/models/whatsnew.hpp>
#endif

#ifdef BROWSER_AVAILABLE
#include <browser-panel.hpp>
#endif

/* widgets/OBSBasic_VirtualCam.cpp */

#include <dialogs/OBSBasicVCamConfig.hpp>

/* widgets/OBSBasic_YouTube.cpp */

#ifdef YOUTUBE_ENABLED
#include <dialogs/OBSYoutubeActions.hpp>
#include <docks/YouTubeAppDock.hpp>
#include <utility/YoutubeApiWrappers.hpp>
#endif

/* widgets/OBSProjector.cpp */

#include <OBSApp.hpp>
#include <components/Multiview.hpp>
#include <utility/platform.hpp>

#include <QScreen>
#include <QWindow>

/* widgets/StatusBarWidget.cpp */
#include "frontend/widgets/StatusBarWidget.hpp"

/* widgets/OBSBasic_Browser.cpp */
#include "frontend/widgets/OBSBasic.hpp"
#ifdef BROWSER_AVAILABLE
#include <dialogs/OBSExtraBrowsers.hpp>
#include <docks/BrowserDock.hpp>
#include <json11.hpp>
#include <qt-wrappers.hpp>
#include <QDir>
#endif
#include <random>
