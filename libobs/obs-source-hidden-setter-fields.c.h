#include "libobs/build/libobs.pch.h"

bool *obs_source_hidden_setter_field(obs_source_t *source);
bool *obs_source_hidden_setter_field(obs_source_t *source)
{
	return &source->temp_removed;
}
