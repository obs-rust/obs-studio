/* Test stand-in for shared/qt/wrappers/qt-wrappers.hpp, for
 * test_plugin_manager: only OBSMessageBox::question(), which
 * PluginManager::open() calls. */

#pragma once

#include <QMessageBox>
#include <QString>
#include <QWidget>

class OBSMessageBox {
public:
	static QMessageBox::StandardButton question(
		QWidget *parent, const QString &title, const QString &text,
		QMessageBox::StandardButtons buttons = QMessageBox::StandardButtons(QMessageBox::Yes | QMessageBox::No),
		QMessageBox::StandardButton defaultButton = QMessageBox::NoButton);
};
