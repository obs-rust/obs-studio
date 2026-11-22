#pragma once

#include <utility/dock-window.hpp>

#include <QDockWidget>

class QCloseEvent;
class QShowEvent;
class QString;

class OBSDock : public QDockWidget {
	Q_OBJECT

public:
	inline OBSDock(QWidget *parent = nullptr) : QDockWidget(parent) { DockWindow::SetUp(this); }
	inline OBSDock(const QString &title, QWidget *parent = nullptr) : QDockWidget(title, parent)
	{
		DockWindow::SetUp(this);
	}

	virtual void closeEvent(QCloseEvent *event);
	virtual void showEvent(QShowEvent *event);
};
