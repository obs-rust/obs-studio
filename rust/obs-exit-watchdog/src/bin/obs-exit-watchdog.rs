//! The watchdog process that OBS starts on Windows (#4589): see
//! `obs_exit_watchdog::helper`. It loads no plugins and no OBS host code.

fn main() {
    std::process::exit(obs_exit_watchdog::run_helper());
}
