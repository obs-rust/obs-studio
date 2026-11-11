/* Precompiled header for the original matrix entry points' private context seam. */
#pragma once
#include "libobs/build/libobs-graphics.pch.h"

graphics_t *obs_graphics_matrix_context(const char *function);
#define thread_graphics gs_get_context()
#define gs_valid(function) (obs_graphics_matrix_context(function) != NULL)
