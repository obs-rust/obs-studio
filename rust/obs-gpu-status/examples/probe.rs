/// Print this host's GPUs, then the render adapter for the render node given
/// as the first argument (e.g. `/dev/dri/renderD128`), or for none.
fn main() {
    for d in obs_gpu_status::platform::system_devices() {
        println!("{d:?}");
    }
    let node = std::env::args().nth(1);
    println!(
        "render adapter for {node:?}: {:?}",
        obs_gpu_status::platform::render_adapter(node.as_deref())
    );
}
