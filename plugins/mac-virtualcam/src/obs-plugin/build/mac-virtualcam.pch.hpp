/* Precompiled header (BUILD.md) for the plugins/mac-virtualcam/src/obs-plugin unity groups
 * mac-virtualcam.macos. Objective-C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* In the unity group the Objective-C++ files are included headers, where
 * clang checks nullability completeness: a file that annotates one pointer
 * (through an NS_ASSUME_NONNULL header) would have to annotate them all,
 * which it never had to as its own main file. */
#pragma clang diagnostic ignored "-Wnullability-completeness"

/* plugin-main.mm (the target builds with -fmodules) */
@import CoreMediaIO;
@import SystemExtensions;

/* OBSDALMachServer.mm */
#import "plugins/mac-virtualcam/src/obs-plugin/OBSDALMachServer.h"
#include <obs-module.h>
#include "MachProtocol.h"
#include "plugins/mac-virtualcam/src/obs-plugin/Defines.h"
#include <fcntl.h>
#include <mach/mach_time.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include "vmic-install.h"
#include "vmic-shm.h"
