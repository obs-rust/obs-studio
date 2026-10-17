/// Print this host's GPUs, then the render adapter for the hint given as the
/// first argument: a render node (`/dev/dri/renderD128`, Linux) or an
/// adapter index (`0`, Windows). Without an argument, no hint.
fn main() {
    for d in obs_gpu_status::platform::system_devices() {
        println!("{d:?}");
    }
    let arg = std::env::args().nth(1);
    let hint = match arg.as_deref().map(|a| (a, a.parse::<u32>())) {
        Some((_, Ok(index))) => obs_gpu_status::RenderHint {
            adapter_index: Some(index),
            ..Default::default()
        },
        Some((node, Err(_))) => obs_gpu_status::RenderHint {
            render_node: Some(node),
            ..Default::default()
        },
        None => obs_gpu_status::RenderHint::default(),
    };
    let adapter = obs_gpu_status::platform::render_adapter(hint);
    match adapter {
        obs_gpu_status::RenderAdapter::RegistryId(id) | obs_gpu_status::RenderAdapter::Luid(id) => {
            println!("render adapter for {hint:?}: {adapter:?} = {id:#x}");
        }
        _ => println!("render adapter for {hint:?}: {adapter:?}"),
    }
    let start = std::time::Instant::now();
    let utilization = obs_gpu_status::platform::gpu_utilization(hint);
    let first = start.elapsed();
    let start = std::time::Instant::now();
    let again = obs_gpu_status::platform::gpu_utilization(hint);
    println!(
        "GPU utilization: {utilization:?} (first call {first:.2?}, with NVML load), {again:?} ({:.2?})",
        start.elapsed()
    );
    // The `GPU: x%` label's readings (#1025): the first source with one,
    // three ticks one second apart, as the frontend's sampler thread takes
    // them (PDH primes its query on the first tick, #1022).
    let mut sampler =
        obs_gpu_status::usage::Sampler::new(obs_gpu_status::platform::usage_sources());
    for tick in 0..3 {
        if tick > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        let start = std::time::Instant::now();
        let sample = sampler.sample(&adapter);
        println!("label sample: {sample:?} ({:.2?})", start.elapsed());
    }
    // PIDs to read the share of (comma-separated, after the hint; default:
    // this process, which uses no GPU), and how many readings to take two
    // seconds apart in one NVML session, as a frontend polls (default 1).
    let pids: Vec<u32> = std::env::args()
        .nth(2)
        .map(|a| a.split(',').filter_map(|p| p.parse().ok()).collect())
        .unwrap_or_else(|| vec![std::process::id()]);
    let readings: u32 = std::env::args()
        .nth(3)
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    let adapter = obs_gpu_status::platform::render_adapter(hint);
    let Some(nvml) = obs_gpu_status::platform::nvml() else {
        return;
    };
    for i in 0..readings {
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let shares: Vec<String> = pids
            .iter()
            .map(|&pid| format!("{pid}={:?}", nvml.process_utilization(&adapter, pid)))
            .collect();
        println!(
            "{:?} shares: {}",
            nvml.utilization(&adapter),
            shares.join(" ")
        );
    }
}
