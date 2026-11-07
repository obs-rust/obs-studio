#include "libobs/build/libobs.pch.h"

struct obs_encoder_metadata_fields obs_encoder_metadata_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_metadata_fields){encoder ? &encoder->info.codec : NULL,
						    encoder ? &encoder->info.type : NULL,
						    encoder ? &encoder->orig_info.id : NULL,
						    encoder ? &encoder->orig_info.type_data : NULL,
						    encoder ? &encoder->orig_info.caps : NULL};
}
