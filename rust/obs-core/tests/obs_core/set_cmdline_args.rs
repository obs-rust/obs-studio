//! Safe mirrors of test_set_cmdline_args.c (and the setter calls of
//! test_core_cmdline_getter.c) over a host that keeps the stored command
//! line and its `bmalloc` blocks as plain memory. Plus what the C tests
//! cannot see: the stored count is read once, nothing else is touched once
//! it is set, the count is stored before any argument is read, each length
//! is read once to size the block and once to copy, and the block is exactly
//! the table and the strings.
use obs_core::set_cmdline_args::{CommandLine, set_cmdline_args};

const PTR: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    ReadArgc,
    StoreArgc(i32),
    ArgLen(usize),
    Allocate(usize),
    Copy(usize, usize, usize),
    Terminate(usize),
}

/// One `bmalloc` block: its size, the table slots written (offsets into the
/// block, `None` for NULL) and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Block {
    size: usize,
    slots: Vec<Option<Option<usize>>>,
    bytes: Vec<Option<u8>>,
}

impl Block {
    /// The string a slot points at, up to its terminator.
    fn string(&self, index: usize) -> Vec<u8> {
        let start = self.slots[index].unwrap().unwrap();
        self.bytes[start..]
            .iter()
            .map(|byte| byte.unwrap())
            .take_while(|&byte| byte != 0)
            .collect()
    }
}

#[derive(Default)]
struct Host {
    argc: i32,
    /// The stored `argv`: an index into `blocks`, or NULL.
    argv: Option<usize>,
    blocks: Vec<Block>,
    input: Vec<Vec<u8>>,
    events: Vec<Event>,
}

impl Host {
    fn call(&mut self, input: &[&[u8]]) {
        self.call_with(i32::try_from(input.len()).unwrap(), input);
    }

    fn call_with(&mut self, argc: i32, input: &[&[u8]]) {
        self.input = input.iter().map(|arg| arg.to_vec()).collect();
        self.events.clear();
        set_cmdline_args(self, argc, PTR);
    }

    fn stored(&self) -> &Block {
        &self.blocks[self.argv.unwrap()]
    }
}

impl CommandLine for Host {
    fn stored_argc(&mut self) -> i32 {
        self.events.push(Event::ReadArgc);
        self.argc
    }
    fn store_argc(&mut self, argc: i32) {
        self.events.push(Event::StoreArgc(argc));
        self.argc = argc;
    }
    fn arg_len(&mut self, index: usize) -> usize {
        self.events.push(Event::ArgLen(index));
        self.input[index].len()
    }
    fn allocate(&mut self, size: usize) {
        self.events.push(Event::Allocate(size));
        self.blocks.push(Block {
            size,
            slots: vec![None; size],
            bytes: vec![None; size],
        });
        self.argv = Some(self.blocks.len() - 1);
    }
    fn copy_arg(&mut self, index: usize, offset: usize, len: usize) {
        self.events.push(Event::Copy(index, offset, len));
        let mut arg = self.input[index].clone();
        arg.push(0);
        assert_eq!(arg.len(), len);
        let block = &mut self.blocks[self.argv.unwrap()];
        assert!(offset + len <= block.size, "copy past the block");
        block.slots[index] = Some(Some(offset));
        for (at, byte) in arg.into_iter().enumerate() {
            block.bytes[offset + at] = Some(byte);
        }
    }
    fn terminate(&mut self, index: usize) {
        self.events.push(Event::Terminate(index));
        let block = &mut self.blocks[self.argv.unwrap()];
        block.slots[index] = Some(None);
    }
}

/// The block a call with `input` stores: the table, then each string.
fn expect_block(host: &Host, input: &[&[u8]]) {
    let block = host.stored();
    let table = (input.len() + 1) * PTR;
    let strings: usize = input.iter().map(|arg| arg.len() + 1).sum();
    assert_eq!(block.size, table + strings);
    let mut next = table;
    for (index, arg) in input.iter().enumerate() {
        assert_eq!(block.slots[index], Some(Some(next)), "argv[{index}]");
        assert_eq!(block.string(index), *arg);
        next += arg.len() + 1;
    }
    assert_eq!(block.slots[input.len()], Some(None), "argv[argc]");
    assert_eq!(next, block.size);
    // Every byte of the block was written, so nothing is left over.
    assert!(block.slots[..=input.len()].iter().all(Option::is_some));
    assert!(block.bytes[table..].iter().all(Option::is_some));
}

// test_set_cmdline_args.c: zero_count_stores_empty_table
#[test]
fn zero_count_stores_empty_table() {
    let mut host = Host::default();
    assert_eq!((host.argc, host.argv), (0, None));

    host.call(&[]);
    assert_eq!(host.blocks.len(), 1, "one allocation");
    assert_eq!(host.argc, 0);
    assert_eq!(host.stored().size, PTR);
    assert_eq!(host.stored().slots[0], Some(None), "argv[0] NULL");
    expect_block(&host, &[]);
}

// test_set_cmdline_args.c: later_call_copies_into_one_block
#[test]
fn later_call_copies_into_one_block() {
    let mut host = Host::default();
    host.call(&[]);
    assert_eq!(host.argc, 0, "a zero count counts as unset");

    let input: &[&[u8]] = &[b"obs", b"--startreplaybuffer", b"", b"--scene=\xc3\xa9 x"];
    host.call(input);
    assert_eq!(host.blocks.len(), 2, "one new block, the empty table stays");
    assert_eq!(host.argv, Some(1));
    assert_eq!(host.argc, 4);
    expect_block(&host, input);
}

// test_set_cmdline_args.c: shutdown_frees_the_copy (the count stays, so a
// later call is ignored and makes no new copy); test_core_cmdline_getter.c's
// ignored second call.
#[test]
fn set_count_ignores_later_calls() {
    let mut host = Host::default();
    let input: &[&[u8]] = &[b"obs", b"--verbose"];
    host.call(input);
    let blocks = host.blocks.clone();

    host.call(&[b"later"]);
    assert_eq!(host.events, [Event::ReadArgc], "nothing else is touched");
    assert_eq!((host.argc, host.argv), (2, Some(0)));
    assert_eq!(host.blocks, blocks);
    expect_block(&host, input);
}

// test_core_cmdline_getter.c: the stored copy keeps every byte, empty and
// non-UTF-8 arguments included.
#[test]
fn copies_every_byte() {
    let mut host = Host::default();
    let input: &[&[u8]] = &[b"obs", b"", b"--profile=\xc3\xa9", b"\xff\x80"];
    host.call(input);
    assert_eq!(host.argc, 4);
    expect_block(&host, input);
}

#[test]
fn reads_and_writes_in_the_original_order() {
    let mut host = Host::default();
    let input: &[&[u8]] = &[b"ab", b"", b"cde"];
    host.call(input);
    let table = 4 * PTR;
    assert_eq!(
        host.events,
        [
            Event::ReadArgc,
            Event::StoreArgc(3),
            Event::ArgLen(0),
            Event::ArgLen(1),
            Event::ArgLen(2),
            Event::Allocate(table + 3 + 1 + 4),
            Event::ArgLen(0),
            Event::Copy(0, table, 3),
            Event::ArgLen(1),
            Event::Copy(1, table + 3, 1),
            Event::ArgLen(2),
            Event::Copy(2, table + 4, 4),
            Event::Terminate(3),
        ]
    );
}

#[test]
fn zero_count_reads_no_argument() {
    let mut host = Host::default();
    host.call(&[]);
    assert_eq!(
        host.events,
        [
            Event::ReadArgc,
            Event::StoreArgc(0),
            Event::Allocate(PTR),
            Event::Terminate(0),
        ]
    );
}

#[test]
fn any_non_zero_count_counts_as_set() {
    for stored in [i32::MIN, -1, 1, i32::MAX] {
        let mut host = Host {
            argc: stored,
            ..Host::default()
        };
        host.call(&[b"obs"]);
        assert_eq!(host.events, [Event::ReadArgc], "{stored}");
        assert_eq!((host.argc, host.argv), (stored, None));
        assert!(host.blocks.is_empty());
    }
}

#[test]
fn negative_count_stores_the_count_only() {
    for argc in [-1, -2, i32::MIN] {
        let mut host = Host::default();
        host.call_with(argc, &[]);
        assert_eq!(host.events, [Event::ReadArgc, Event::StoreArgc(argc)]);
        assert_eq!((host.argc, host.argv), (argc, None));
        assert!(host.blocks.is_empty());
    }
}

#[test]
fn pointer_size_sizes_the_table() {
    let mut host = Host::default();
    host.input = vec![b"x".to_vec()];
    set_cmdline_args(&mut host, 1, 4);
    assert_eq!(
        host.events,
        [
            Event::ReadArgc,
            Event::StoreArgc(1),
            Event::ArgLen(0),
            Event::Allocate(2 * 4 + 2),
            Event::ArgLen(0),
            Event::Copy(0, 2 * 4, 2),
            Event::Terminate(1),
        ]
    );
}
