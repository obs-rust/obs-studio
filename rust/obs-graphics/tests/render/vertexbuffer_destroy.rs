use obs_graphics::vertexbuffer_destroy::destroy;

#[test]
fn a_save_hands_its_data_over() {
    let mut trace = vec!["create 4 tex0 flags0"];
    let mut buffers_alive = 1;
    let buffer = Box::new(vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
    destroy(
        Some(buffer),
        Some(|data: Box<Vec<(i32, i32)>>| {
            assert_eq!(*data, [(0, 0), (1, 0), (0, 1), (1, 1)]);
            trace.push("destroy");
            buffers_alive -= 1;
        }),
    );
    assert_eq!(trace.join(";"), "create 4 tex0 flags0;destroy");
    assert_eq!(buffers_alive, 0);
}

#[test]
fn null_buffer_is_silent() {
    destroy(None::<usize>, Some(|_| panic!("null buffer dispatched")));
}

#[test]
fn absent_context_does_not_dispatch() {
    destroy(Some(7), None::<fn(usize)>);
    destroy(None::<usize>, None::<fn(usize)>);
}

#[test]
fn a_new_buffer_draws_and_is_destroyed() {
    let mut trace = vec!["create 4 tex1 flags0", "load", "draw 4 0 0"];
    let mut buffers_alive = 1;
    destroy(
        Some(42),
        Some(|buffer| {
            assert_eq!(buffer, 42);
            trace.push("destroy");
            buffers_alive -= 1;
        }),
    );
    assert_eq!(
        trace.join(";"),
        "create 4 tex1 flags0;load;draw 4 0 0;destroy"
    );
    assert_eq!(buffers_alive, 0);
}
