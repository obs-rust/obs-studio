//! obs-util integration tests: config (one module per former tests/*.rs file, #968).

use crate::common;

mod cf_parser;
mod cf_parser_parity;
mod cf_tokenizer;
mod cf_tokenizer_ffi_bounds;
mod cf_tokenizer_layout;
mod cf_tokenizer_parity;
mod config_file;
mod config_file_close;
mod config_file_duplicates_parity;
mod config_file_load_scaling;
mod config_file_parity;
mod lexer;
mod lexer_layout;
mod lexer_parity;
mod text_lookup;
mod text_lookup_parity;
