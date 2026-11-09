//! C ABI, one module per C file.

pub mod encoder_audio_getters;
pub mod encoder_audio_start;
pub mod encoder_audio_start_frames;
pub mod encoder_bind;
pub mod encoder_counters;
pub mod encoder_dimensions;
pub mod encoder_frame_rate_divisor;
pub mod encoder_gpu_gates;
pub mod encoder_gpu_rescale;
pub mod encoder_gpu_scaling_getters;
pub mod encoder_group_reconfig;
pub mod encoder_last_error;
pub mod encoder_lifecycle;
pub mod encoder_metadata;
pub mod encoder_packet_ts;
pub mod encoder_pause;
pub mod encoder_preferred_color_space;
pub mod encoder_preferred_format;
pub mod encoder_preferred_range;
pub mod encoder_roi;
pub mod encoder_roi_state;
pub mod encoder_scaling_enabled;
pub mod encoder_size;
pub mod encoder_state_queries;
pub mod output_active;
pub mod output_begin;
pub mod output_can_pause;
pub mod output_caption;
pub mod output_caption_text;
pub mod output_delay;
pub mod output_flags;
pub mod output_handler_getters;
pub mod output_identity;
pub mod output_interleave;
pub mod output_mixer_getters;
pub mod output_pause;
pub mod output_pause_queries;
pub mod output_protocol_getter;
pub mod output_protocols;
pub mod output_raw_audio;
pub mod output_raw_audio_trim;
pub mod output_reconnect;
pub mod output_reconnecting;
pub mod output_service_getter;
pub mod output_slots;
pub mod output_start;
pub mod output_stop;
pub mod output_stop_stats;
pub mod output_supported_codecs;
pub mod output_video_checks;
pub mod service;
pub mod service_module_lookup;
pub mod service_state;
pub mod service_weak_acquire;
pub mod service_weak_release;

pub mod weak_encoder_identity;

pub mod output_total_frames;

pub mod output_connect_time;

pub mod output_frames_dropped;
