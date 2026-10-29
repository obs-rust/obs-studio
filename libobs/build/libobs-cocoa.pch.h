/* Precompiled header (BUILD.md) for the libobs unity groups libobs.macos-cocoa, libobs.macos-loader. Objective-C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* obs-cocoa.m */
#include "util/dstr.h"
#include "util/platform.h"
#include "obs.h"
#include "obs-internal.h"
#include "obs-cocoa-keys.h"
#include "obs-host/default-monitoring-device.h"
#include "obs-host/display-srgb-clear.h"
#include "obs-host/graphics-thread.h"
#include "obs-host/module-paths.h"

#include <unistd.h>
#include <sys/types.h>
#include <sys/sysctl.h>

#include <Carbon/Carbon.h>
#include <IOKit/hid/IOHIDDevice.h>
#include <IOKit/hid/IOHIDManager.h>

#import <AppKit/AppKit.h>

/* obs-core-modules-loader-mac.m */
#import "obs-core-modules.h"

#import <obs-internal.h>
#import <obs.h>
#import <util/dstr.h>
#import <util/platform.h>

#import <Foundation/Foundation.h>
