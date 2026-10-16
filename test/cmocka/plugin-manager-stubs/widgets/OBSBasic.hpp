/* Test stand-in for frontend/widgets/OBSBasic.hpp, for test_plugin_manager:
 * only OBSBasic::Get(), which PluginManager::open() calls. */

#pragma once

#include <QWidget>

class OBSBasic : public QWidget {
public:
	static OBSBasic *Get();
};
