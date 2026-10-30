/* Precompiled header (BUILD.md) for the plugins/linux-v4l2 unity groups
 * linux-v4l2.nix-output. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* v4l2-output.c */
#define _GNU_SOURCE

#include <obs-module.h>
#include <util/dstr.h>
#include <util/platform.h>
#include <linux/videodev2.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vcam-format.h"
