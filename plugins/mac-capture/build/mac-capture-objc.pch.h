/* Precompiled header (BUILD.md) for the plugins/mac-capture unity groups
 * mac-capture.objc-macos. Objective-C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* mac-display-capture.m */
#include <stdlib.h>
#include <obs-module.h>
#include <util/threading.h>
#include <pthread.h>

#import <AvailabilityMacros.h>
#import <CoreGraphics/CGDisplayStream.h>
#import <Cocoa/Cocoa.h>

#include "plugins/mac-capture/window-utils.h"

/* mac-sck-audio-capture.m */
#include "plugins/mac-capture/mac-sck-common.h"
#include "plugins/mac-capture/mac-sck-stall.h"
#include "plugins/mac-capture/mac-sck-stop.h"

/* mac-window-capture.m */
#include <util/darray.h>
#include <util/platform.h>

#import <CoreGraphics/CGWindow.h>
