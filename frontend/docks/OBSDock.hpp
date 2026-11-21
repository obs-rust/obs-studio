#pragma once

#include <utility/dock-window.hpp>

#include <QDockWidget>

class QCloseEvent;
class QShowEvent;
class QString;

class OBSDock : public QDockWidget {
	Q_OBJECT

public:
	inline OBSDock(QWidget *parent = nullptr) : QDockWidget(parent) { KeepFloatingDockShown(this); }
	inline OBSDock(const QString &title, QWidget *parent = nullptr) : QDockWidget(title, parent)
	{
		KeepFloatingDockShown(this);
	}

	virtual void closeEvent(QCloseEvent *event);
	virtual void showEvent(QShowEvent *event);
};
