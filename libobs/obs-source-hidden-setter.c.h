#include "libobs/build/libobs.pch.h"

void obs_source_set_hidden(obs_source_t *source, bool hidden)
{
	source->temp_removed = hidden;
}
