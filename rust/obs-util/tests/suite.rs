//! One amalgamated category harness; ordinary cases run in parallel.

#[path = "threading/main.rs"]
mod concurrency;
#[path = "config/main.rs"]
mod config;
#[path = "containers/main.rs"]
mod containers;
#[path = "memory/main.rs"]
mod memory;
#[path = "platform/main.rs"]
mod platform;
#[path = "profiler/main.rs"]
mod profiling;
#[path = "serializer/main.rs"]
mod serializer;
#[path = "strings/main.rs"]
mod strings;

#[path = "../../obs-c-oracle/tests/oracle/main.rs"]
mod oracle;
mod windows_out_dir;
