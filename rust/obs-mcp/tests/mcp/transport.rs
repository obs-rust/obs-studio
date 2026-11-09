use obs_mcp::platform::{endpoint, EndpointOptions};
#[test]
fn explicit_endpoint_is_preserved() {
 let path = std::path::PathBuf::from("/test/control.sock");
 assert_eq!(endpoint(&EndpointOptions { socket:Some(path.clone()) }).unwrap(), path);
}
