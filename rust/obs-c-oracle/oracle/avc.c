/* Test-only oracle: the original libobs/obs-avc.c, unmodified, with its
 * global symbols renamed to oracle_* so it can link next to the Rust
 * implementation. The libobs functions it calls are pointed at the oracle
 * copies of obs-nal.c, array-serializer.c and bitstream.c, so the whole
 * call tree is the original C. */
#define obs_avc_keyframe oracle_obs_avc_keyframe
#define obs_avc_find_startcode oracle_obs_avc_find_startcode
#define obs_parse_avc_packet oracle_obs_parse_avc_packet
#define obs_parse_avc_packet_priority oracle_obs_parse_avc_packet_priority
#define obs_parse_avc_header oracle_obs_parse_avc_header
#define obs_extract_avc_headers oracle_obs_extract_avc_headers
#define obs_nal_find_startcode oracle_obs_nal_find_startcode
#define array_output_serializer_init oracle_array_output_serializer_init
#define bitstream_reader_init oracle_bitstream_reader_init
#define bitstream_reader_read_bits oracle_bitstream_reader_read_bits

#include "obs-avc.c"
