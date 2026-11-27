#pragma once

/* Stand-ins for the main window and application (test_volume_control_uuid):
 * the members of OBSBasic and OBSApp that frontend/components/
 * VolumeControl.cpp.hpp uses. OBSBasic::mixerStatusChanged() is the signal
 * the test watches. */

#include <obs.hpp>
#include <util/config-file.h>

#include <QObject>
#include <QString>
#include <QWidget>

#include <functional>
#include <string>

class undo_stack {
public:
	typedef std::function<void(const std::string &data)> undo_redo_cb;

	void add_action(const QString &, const undo_redo_cb &, const undo_redo_cb &, const std::string &,
			const std::string &, bool = false)
	{
	}
};

class OBSBasic : public QWidget {
	Q_OBJECT

public:
	undo_stack undo_s;

	static OBSBasic *Get();

	config_t *Config() const { return nullptr; }
	obs_weak_source_t *copyFiltersSource() { return nullptr; }

public slots:
	void actionCopyFilters() {}
	void actionPasteFilters() {}
	void actionOpenSourceFilters() {}
	void actionOpenSourceProperties() {}

signals:
	void profileSettingChanged(const std::string &category, const std::string &name);
	void mixerStatusChanged(QString sourceUuid);
};

typedef std::function<void()> VoidFunc;

class OBSApp : public QObject {
public:
	QWidget *GetMainWindow() const { return nullptr; }
	config_t *GetUserConfig() const { return nullptr; }
	void Exec(VoidFunc func) { func(); }
};

OBSApp *App();

/* The lookup key with a %1, for the callers that .arg() a name into it. */
inline QString QTStr(const char *lookupVal)
{
	return QString::fromUtf8(lookupVal) + QStringLiteral(" %1");
}
