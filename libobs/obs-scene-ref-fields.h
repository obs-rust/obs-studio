#pragma once

#include <stdbool.h>

struct obs_source;

/* One-step calls keep source ownership behind the existing libobs contract. */
bool obs_scene_ref_acquire_source(struct obs_source *source);
void obs_scene_ref_release_source(struct obs_source *source);
