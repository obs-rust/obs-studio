/* Precompiled header for the original blend entry points' private context seam. */
#pragma once
#include "libobs/build/libobs-graphics.pch.h"

graphics_t *obs_graphics_blend_context(const char *function);
#define thread_graphics gs_get_context()
#define gs_valid(function) (obs_graphics_blend_context(function) != NULL)
