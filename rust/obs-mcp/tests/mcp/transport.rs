use obs_mcp::platform::{EndpointOptions, endpoint};
#[test]
fn explicit_endpoint_is_preserved() {
    let path = std::path::PathBuf::from("/test/control.sock");
    assert_eq!(
        endpoint(&EndpointOptions {
            socket: Some(path.clone())
        })
        .unwrap(),
        path
    );
}
#[cfg(unix)]
#[test]
fn private_socket_serves_fragmented_and_multiple_clients_and_cleans_up() {
    use obs_mcp::{
        platform::{SocketServer, connect},
        server::Server,
    };
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let options = EndpointOptions {
        socket: Some(directory.path().join("control.sock")),
    };
    let socket = SocketServer::bind(&options).unwrap();
    assert_eq!(
        std::fs::metadata(socket.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(SocketServer::bind(&options).is_err());
    let stop = Arc::new(AtomicBool::new(false));
    struct Guard(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
            self.1.take().unwrap().join().unwrap();
        }
    }
    let flag = stop.clone();
    let _guard = Guard(
        stop,
        Some(std::thread::spawn(move || {
            socket.run(&flag, Server::new).unwrap()
        })),
    );
    let mut a = connect(&options).unwrap();
    let mut b = connect(&options).unwrap();
    a.writer
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,")
        .unwrap();
    b.writer
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n")
        .unwrap();
    let mut line = String::new();
    b.reader.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["id"],
        2
    );
    a.writer.write_all(b"\"method\":\"ping\"}\n").unwrap();
    line.clear();
    a.reader.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["id"],
        1
    );
    drop(a);
    drop(b);
    drop(_guard);
    assert!(!options.socket.unwrap().exists());
}
#[cfg(unix)]
#[test]
fn refuses_non_socket_and_symlink() {
    use obs_mcp::platform::SocketServer;
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let file = dir.path().join("file");
    std::fs::write(&file, b"keep").unwrap();
    assert!(
        SocketServer::bind(&EndpointOptions {
            socket: Some(file.clone())
        })
        .is_err()
    );
    let link = dir.path().join("link");
    symlink(&file, &link).unwrap();
    assert!(SocketServer::bind(&EndpointOptions { socket: Some(link) }).is_err());
    assert_eq!(std::fs::read(file).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn shutdown_discards_requests_already_buffered_after_cancellation() {
    use obs_mcp::{
        platform::{SocketServer, connect},
        server::{Server, ToolAnnotations, ToolError, ToolRegistry, TypedTool},
        tools::EmptyInput,
    };
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };
    struct Cancel {
        stop: Arc<AtomicBool>,
        calls: Arc<AtomicUsize>,
    }
    impl TypedTool for Cancel {
        type Input = EmptyInput;
        type Output = EmptyInput;
        const NAME: &'static str = "cancel_test";
        const ANNOTATIONS: ToolAnnotations = ToolAnnotations::MUTATING;
        fn description(&self) -> &'static str {
            "Cancel the test server"
        }
        fn call(&self, _: EmptyInput) -> Result<EmptyInput, ToolError> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            self.stop.store(true, Ordering::Release);
            Ok(EmptyInput {})
        }
    }
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let options = EndpointOptions {
        socket: Some(directory.path().join("control.sock")),
    };
    let socket = SocketServer::bind(&options).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let flag = stop.clone();
    let count = calls.clone();
    struct Guard(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
            self.1.take().unwrap().join().unwrap();
        }
    }
    let thread = std::thread::spawn(move || {
        socket
            .run(&flag, || {
                let mut tools = ToolRegistry::default();
                tools.register(Box::new(Cancel {
                    stop: flag.clone(),
                    calls: count.clone(),
                }));
                Server::with_tools(tools)
            })
            .unwrap()
    });
    let guard = Guard(stop, Some(thread));
    let mut connection = connect(&options).unwrap();
    connection.writer.write_all(concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"cancel_test\",\"arguments\":{}}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"cancel_test\",\"arguments\":{}}}\n"
    ).as_bytes()).unwrap();
    let mut reply = String::new();
    connection.reader.read_line(&mut reply).unwrap();
    drop(guard);
    assert_eq!(calls.load(Ordering::Acquire), 1);
}
