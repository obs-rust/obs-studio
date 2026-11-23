use obs_vst3::bus::{BusPlan, Description, Error, Kind, Selection};
use obs_vst3::parameters::{Point, Queue, Restart};
use obs_vst3::state::{SeekOrigin, StateStream};

const MONO: u64 = 1 << 19;
const STEREO: u64 = 3;

fn bus(kind: Kind, channels: usize, arrangement: Option<u64>) -> Description {
    Description {
        kind,
        channels,
        arrangement,
    }
}

#[test]
fn first_main_and_compatible_auxiliary_buses_are_selected() {
    let input = [
        bus(Kind::Auxiliary, 4, Some(15)),
        bus(Kind::Auxiliary, 1, Some(MONO)),
        bus(Kind::Main, 1, Some(MONO)),
        bus(Kind::Main, 6, Some(63)),
        bus(Kind::Auxiliary, 2, Some(STEREO)),
    ];
    let output = [
        bus(Kind::Auxiliary, 1, Some(MONO)),
        bus(Kind::Main, 2, Some(STEREO)),
        bus(Kind::Main, 6, Some(63)),
    ];
    let plan = BusPlan::select(&input, &output, STEREO).unwrap();
    assert_eq!(plan.main_input, 2);
    assert_eq!(plan.main_output, 1);
    assert_eq!(plan.sidechain, Some(1));
    assert_eq!(
        plan.input,
        [
            Selection {
                enabled: false,
                arrangement: 15
            },
            Selection {
                enabled: true,
                arrangement: MONO
            },
            Selection {
                enabled: true,
                arrangement: STEREO
            },
            Selection {
                enabled: false,
                arrangement: 63
            },
            Selection {
                enabled: false,
                arrangement: STEREO
            },
        ]
    );
    assert_eq!(
        plan.output,
        [
            Selection {
                enabled: false,
                arrangement: MONO
            },
            Selection {
                enabled: true,
                arrangement: STEREO
            },
            Selection {
                enabled: false,
                arrangement: 63
            },
        ]
    );
    assert!(!plan.matches_active_arrangements(&input, &output));
    let mut negotiated = input;
    negotiated[2].arrangement = Some(STEREO);
    assert!(plan.matches_active_arrangements(&negotiated, &output));
}

#[test]
fn missing_main_buses_and_failed_arrangement_queries_are_rejected() {
    let main = bus(Kind::Main, 2, Some(STEREO));
    let auxiliary = bus(Kind::Auxiliary, 6, None);
    assert_eq!(
        BusPlan::select(&[], &[main], STEREO),
        Err(Error::MissingMainInput)
    );
    assert_eq!(
        BusPlan::select(&[main], &[], STEREO),
        Err(Error::MissingMainOutput)
    );
    assert_eq!(
        BusPlan::select(&[main, auxiliary], &[main], STEREO),
        Err(Error::ArrangementUnavailable),
    );
}

#[test]
fn state_stream_reads_short_and_preserves_saved_cursor_length() {
    let mut stream = StateStream::from_bytes(vec![0, 0, 0, 0x3f]);
    let mut read = [0xff; 8];
    assert_eq!(stream.read(&mut read), 4);
    assert_eq!(read, [0, 0, 0, 0x3f, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(stream.seek(-2, SeekOrigin::End).unwrap(), 2);
    assert_eq!(stream.write(&[0x80]).unwrap(), 1);
    assert_eq!(stream.position(), 3);
    assert_eq!(stream.saved_bytes(), [0, 0, 0x80]);
    assert_eq!(stream.seek(0, SeekOrigin::Start).unwrap(), 0);
    assert_eq!(stream.read(&mut read), 4);
    assert_eq!(&read[..4], &[0, 0, 0x80, 0x3f]);
    assert_eq!(stream.seek(3, SeekOrigin::Current).unwrap(), 7);
    assert_eq!(stream.read(&mut read), 0);
    assert_eq!(stream.position(), 4);
}

#[test]
fn automation_points_are_sorted_and_replace_duplicate_offsets() {
    let mut queue = Queue::default();
    let point = |offset, value| Point { offset, value };
    assert_eq!(queue.insert(point(15, 0.25)), 0);
    assert_eq!(queue.insert(point(3, 0.5)), 0);
    assert_eq!(queue.insert(point(10, 1.0)), 1);
    assert_eq!(queue.insert(point(10, 0.75)), 1);
    assert_eq!(
        queue.points(),
        &[point(3, 0.5), point(10, 0.75), point(15, 0.25)]
    );
}

#[test]
fn restart_handles_latency_and_parameter_values() {
    assert_eq!(Restart::decode(1), None);
    assert_eq!(
        Restart::decode(1 << 3),
        Some(Restart {
            latency_changed: true,
            values_changed: false
        }),
    );
    assert_eq!(
        Restart::decode((1 << 2) | (1 << 3) | (1 << 1)),
        Some(Restart {
            latency_changed: true,
            values_changed: true
        }),
    );
}
