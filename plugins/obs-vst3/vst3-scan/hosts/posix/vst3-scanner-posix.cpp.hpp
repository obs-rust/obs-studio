// SPDX-License-Identifier: GPL-3.0-or-later
#include "plugins/obs-vst3/vst3-scan/build/obs-vst3-scanner.posix.pch.hpp"

int main(int argc, char *argv[])
{
	return vst3_scanner_main(argc > 1 ? argv[1] : nullptr);
}
