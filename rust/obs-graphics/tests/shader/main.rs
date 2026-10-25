//! obs-graphics integration tests: shader (one module per former tests/*.rs file, #968).

mod effect_lookups;
mod effect_lookups_layout;
mod effect_lookups_parity;
mod effect_params;
mod effect_params_parity;
mod effect_parser_lifecycle;
mod effect_passes;
mod effect_passes_parity;
mod shader_param_names;
mod shader_param_names_parity;

mod effect_parser_lifecycle_parity;

mod shader_sampler_convert;

mod shader_sampler_convert_parity;

use std::sync::Mutex;

/// Held by every test here that swaps the process-wide log handler
/// (`base_set_log_handler`) or uses the effect pass device stand-in, so two
/// tests never interleave their save and restore.
pub(crate) static LOG_HANDLER: Mutex<()> = Mutex::new(());
