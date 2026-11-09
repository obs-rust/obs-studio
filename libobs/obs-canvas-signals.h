/* Private canvas signal seam; never part of the installed API. */
#pragma once

void canvas_dosignal(obs_canvas_t *canvas, const char *signal_obs, const char *signal_canvas);
void canvas_dosignal_source(const char *signal, obs_canvas_t *canvas, obs_source_t *source);
bool obs_canvas_signals_private(const obs_canvas_t *canvas);
