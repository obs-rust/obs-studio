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
}
