//! obs-core integration tests, one module per extracted C file. A module of
//! the core category binary (obs-tests-core), not a binary of its own.

mod audio_discard;
mod audio_discard_parity;
mod audio_min_ts;
mod audio_min_ts_parity;
mod audio_mix;
mod audio_mix_parity;
mod canvas_draw;
mod canvas_draw_parity;
mod context_names;
mod context_names_parity;
mod module_locale;
mod module_locale_parity;
mod module_names;
mod module_names_parity;
mod module_paths;
mod module_paths_parity;
mod module_source_rules;
mod module_source_rules_parity;
mod module_type_rules;
mod module_type_rules_parity;
mod source_load;
mod source_load_parity;
mod source_profiler;
mod source_profiler_parity;
mod video_copy_plan;
mod video_copy_plan_parity;
mod video_output_step;
mod video_output_step_parity;
mod video_plan;
mod video_plan_parity;
mod view;
mod view_parity;
