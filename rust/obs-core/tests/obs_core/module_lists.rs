//! Tier 1: the safe core of `libobs/obs-module-lists.c.h`.
//!
//! The first eight tests mirror `test/cmocka/test_module_lists.c` one to
//! one, with the same names, in the same order, with the same ids, names
//! and paths, over a model core: [`Model`] stands for the core libobs
//! starts, and a module made with `obs_create_disabled_module()` is a model
//! module that is not on the core's list of loaded modules. What the C test
//! counts with `bnum_allocs()` (one copy per string, nothing with nothing
//! to push) is here what the model's lists hold; the allocations themselves
//! are the C shim's, and the parity test compares them with the C.
//!
//! Cases marked "characterized, not endorsed" are odd but real and kept as
//! the C has them. The tests after the eight pin the order of the reads,
//! the one intentional difference (`obs_log_loaded_modules()` with no core,
//! which the C test does not call because it crashes), and proptests over
//! any arguments.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use obs_core::module_lists::{
    HEADING, Line, Lists, MODULE_FORMAT, NameList, TypeList, add_module_path, add_name, add_type,
    config_path, find_file, log_loaded_modules,
};
use obs_core::module_paths::{GlobEntry, ModuleFs};
use proptest::prelude::*;

const TYPE_LISTS: [TypeList; 4] = [
    TypeList::Sources,
    TypeList::Outputs,
    TypeList::Encoders,
    TypeList::Services,
];

/// What the model was asked, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Read {
    HasCore,
    FirstModule,
    ModuleConfigPath,
    Next(usize),
    DataPath(usize),
    ModuleName(usize),
}

#[derive(Clone, Debug, Default)]
struct ModelModule {
    data_path: Option<Vec<u8>>,
    name: Option<Vec<u8>>,
    next: Option<usize>,
    types: BTreeMap<TypeList, Vec<Option<Vec<u8>>>>,
}

/// A core and its modules. `modules[i]` is module `i`; the loaded ones are
/// the chain from `first`.
#[derive(Debug, Default)]
struct Model {
    core: bool,
    first: Option<usize>,
    config_path: Option<Vec<u8>>,
    modules: Vec<ModelModule>,
    paths: Vec<(Vec<u8>, Vec<u8>)>,
    names: BTreeMap<NameList, Vec<Vec<u8>>>,
    log: Vec<Line<usize>>,
    reads: RefCell<Vec<Read>>,
}

impl Model {
    /// A core as `obs_startup("en-US", "module-lists-config", NULL)` makes
    /// it: no module loaded.
    fn started() -> Self {
        Self {
            core: true,
            config_path: Some(b"module-lists-config".to_vec()),
            ..Self::default()
        }
    }

    /// `obs_create_disabled_module()`: a module that is not loaded.
    fn disabled_module(&mut self, name: &str, data_path: &str) -> usize {
        self.modules.push(ModelModule {
            data_path: Some(data_path.as_bytes().to_vec()),
            name: Some(name.as_bytes().to_vec()),
            ..ModelModule::default()
        });
        self.modules.len() - 1
    }

    /// A loaded module, at the front of the core's list, as
    /// `obs_init_module()` puts one.
    fn loaded_module(&mut self, name: &str) -> usize {
        let m = self.disabled_module(name, "nodata");
        self.modules[m].next = self.first;
        self.first = Some(m);
        m
    }

    fn read(&self, read: Read) {
        self.reads.borrow_mut().push(read);
    }

    fn types(&self, module: usize, list: TypeList) -> &[Option<Vec<u8>>] {
        self.modules[module]
            .types
            .get(&list)
            .map_or(&[], Vec::as_slice)
    }

    fn names(&self, list: NameList) -> &[Vec<u8>] {
        self.names.get(&list).map_or(&[], Vec::as_slice)
    }

    /// Every list's entries, to check that a call changed nothing.
    fn pushed(&self) -> usize {
        self.paths.len()
            + self.names.values().map(Vec::len).sum::<usize>()
            + self
                .modules
                .iter()
                .flat_map(|m| m.types.values())
                .map(Vec::len)
                .sum::<usize>()
    }

    /// The type `obs_source_get_module()` and its siblings find: the first
    /// module whose `list` names `id`.
    fn type_module(&self, list: TypeList, id: &str) -> Option<usize> {
        (0..self.modules.len()).find(|&m| {
            self.types(m, list)
                .iter()
                .any(|e| e.as_deref() == Some(id.as_bytes()))
        })
    }
}

impl Lists for Model {
    type Module = usize;

    fn has_core(&self) -> bool {
        self.read(Read::HasCore);
        self.core
    }

    fn first_module(&self) -> Option<usize> {
        self.read(Read::FirstModule);
        assert!(self.core, "first_module read with no core");
        self.first
    }

    fn module_config_path(&self) -> Option<&[u8]> {
        self.read(Read::ModuleConfigPath);
        self.config_path.as_deref()
    }

    fn next(&self, module: usize) -> Option<usize> {
        self.read(Read::Next(module));
        self.modules[module].next
    }

    fn data_path(&self, module: usize) -> Option<&[u8]> {
        self.read(Read::DataPath(module));
        self.modules[module].data_path.as_deref()
    }

    fn module_name(&self, module: usize) -> Option<&[u8]> {
        self.read(Read::ModuleName(module));
        self.modules[module].name.as_deref()
    }

    fn log(&mut self, line: Line<usize>) {
        self.log.push(line);
    }

    fn push_type(&mut self, module: usize, list: TypeList, id: Option<&[u8]>) {
        self.modules[module]
            .types
            .entry(list)
            .or_default()
            .push(id.map(<[u8]>::to_vec));
    }

    fn push_name(&mut self, list: NameList, name: &[u8]) {
        assert!(self.core, "push_name with no core");
        self.names.entry(list).or_default().push(name.to_vec());
    }

    fn push_path(&mut self, bin: &[u8], data: &[u8]) {
        assert!(self.core, "push_path with no core");
        self.paths.push((bin.to_vec(), data.to_vec()));
    }
}

/// A file system of files that exist.
#[derive(Default)]
struct FakeFs {
    files: BTreeSet<Vec<u8>>,
}

impl ModuleFs for FakeFs {
    fn file_exists(&self, path: &[u8]) -> bool {
        self.files.contains(path)
    }
    fn glob(&self, _pattern: &[u8]) -> Option<Vec<GlobEntry>> {
        None
    }
}

fn s(text: &str) -> Option<&[u8]> {
    Some(text.as_bytes())
}

/* ---------------------------------------------------------------- no core */

/// Mirrors `no_core_calls`: with no core, the calls that check it do
/// nothing, and the ones that check only the module return `None` for no
/// module.
#[test]
fn no_core_calls() {
    let mut core = Model::default();
    let fs = FakeFs::default();

    add_module_path(&mut core, s("bin"), s("data"));
    add_name(&mut core, NameList::Safe, s("safe"));
    add_name(&mut core, NameList::Disabled, s("disabled"));
    for list in TYPE_LISTS {
        add_type(&mut core, None, list, s("id"));
    }
    assert_eq!(find_file(&core, &fs, None, s("file")), None);
    assert_eq!(config_path(&core, None, s("file")), None);
    assert_eq!(core.pushed(), 0);
    assert_eq!(*core.reads.borrow(), vec![Read::HasCore; 3]);
}

/* ------------------------------------------------------------------ core */

/// Mirrors `log_no_loaded_modules`: no module loaded, only the heading. A
/// disabled module is not a loaded one, so it adds no line.
#[test]
fn log_no_loaded_modules() {
    let mut core = Model::started();
    log_loaded_modules(&mut core);
    assert_eq!(core.log, vec![Line::Heading]);
    assert_eq!(HEADING, "  Loaded Modules:");

    core.disabled_module("module_lists_stub_logged", "nodata");
    core.log.clear();
    log_loaded_modules(&mut core);
    assert_eq!(core.log, vec![Line::Heading]);
}

/// Mirrors `add_types_no_module`: with no module, nothing is pushed, and
/// no module names the id.
#[test]
fn add_types_no_module() {
    let mut core = Model::started();
    core.disabled_module("module_lists_stub", "nodata");
    for list in TYPE_LISTS {
        add_type(&mut core, None, list, s("module_lists_null"));
    }
    add_type(&mut core, None, TypeList::Sources, None);
    assert_eq!(core.pushed(), 0);
    for list in TYPE_LISTS {
        assert_eq!(core.type_module(list, "module_lists_null"), None);
    }
    assert!(core.reads.borrow().is_empty());
}

/// Mirrors `add_types_to_module`: each list gets its own copy of the id,
/// and only that kind's lookup names the module.
#[test]
fn add_types_to_module() {
    let mut core = Model::started();
    let m = core.disabled_module("module_lists_stub_types", "nodata");

    let mut source = b"module_lists_source".to_vec();
    let mut output = b"module_lists_output".to_vec();
    let mut encoder = b"module_lists_encoder".to_vec();
    let mut service = b"module_lists_service".to_vec();
    add_type(&mut core, Some(m), TypeList::Sources, Some(&source));
    add_type(&mut core, Some(m), TypeList::Outputs, Some(&output));
    add_type(&mut core, Some(m), TypeList::Encoders, Some(&encoder));
    add_type(&mut core, Some(m), TypeList::Services, Some(&service));
    for list in TYPE_LISTS {
        assert_eq!(core.types(m, list).len(), 1);
    }

    // Copies: the caller's strings can change.
    for id in [&mut source, &mut output, &mut encoder, &mut service] {
        id[0] = b'X';
    }

    assert_eq!(
        core.type_module(TypeList::Sources, "module_lists_source"),
        Some(m)
    );
    assert_eq!(
        core.type_module(TypeList::Outputs, "module_lists_output"),
        Some(m)
    );
    assert_eq!(
        core.type_module(TypeList::Encoders, "module_lists_encoder"),
        Some(m)
    );
    assert_eq!(
        core.type_module(TypeList::Services, "module_lists_service"),
        Some(m)
    );

    assert_eq!(
        core.type_module(TypeList::Sources, "Xodule_lists_source"),
        None
    );
    assert_eq!(
        core.type_module(TypeList::Sources, "module_lists_output"),
        None
    );
    assert_eq!(
        core.type_module(TypeList::Outputs, "module_lists_source"),
        None
    );
    assert_eq!(
        core.type_module(TypeList::Encoders, "module_lists_service"),
        None
    );
    assert_eq!(
        core.type_module(TypeList::Services, "module_lists_encoder"),
        None
    );

    // The same id again: a second entry.
    add_type(
        &mut core,
        Some(m),
        TypeList::Sources,
        s("module_lists_source"),
    );
    assert_eq!(
        core.types(m, TypeList::Sources),
        [
            Some(b"module_lists_source".to_vec()),
            Some(b"module_lists_source".to_vec())
        ]
    );
    assert_eq!(
        core.type_module(TypeList::Sources, "module_lists_source"),
        Some(m)
    );
}

/// Mirrors `find_module_file`: the module's data path, '/', the file, when
/// the file exists; `None` when it doesn't, or with no module.
#[test]
fn find_module_file() {
    let mut core = Model::started();
    let fs = FakeFs {
        files: [b"module_lists_data/present.txt".to_vec()].into(),
    };
    let m = core.disabled_module("module_lists_stub_files", "module_lists_data");

    let found = find_file(&core, &fs, Some(m), s("present.txt"));
    let missing = find_file(&core, &fs, Some(m), s("missing.txt"));
    let none = find_file(&core, &fs, None, s("present.txt"));

    assert_eq!(
        found.as_deref(),
        Some(&b"module_lists_data/present.txt"[..])
    );
    assert_eq!(missing, None);
    assert_eq!(none, None);
}

/// Mirrors `module_config_path`: the core's module config path, '/', the
/// module's name, '/', the file. `None` only with no module.
#[test]
fn module_config_path() {
    let mut core = Model::started();
    let m = core.disabled_module("module_lists_stub", "nodata");

    assert_eq!(
        config_path(&core, Some(m), s("settings.json")).as_deref(),
        Some(&b"module-lists-config/module_lists_stub/settings.json"[..])
    );

    // No file: the module's config directory and a trailing '/'
    // (characterized, not endorsed).
    assert_eq!(
        config_path(&core, Some(m), None).as_deref(),
        Some(&b"module-lists-config/module_lists_stub/"[..])
    );

    assert_eq!(config_path(&core, None, s("settings.json")), None);
}

/// Mirrors `add_list_entries`: nothing without both paths; with both, a
/// copy of each. A safe or disabled name: nothing without one; with one, a
/// copy. Empty strings are copied like any other.
#[test]
fn add_list_entries() {
    let mut core = Model::started();

    add_module_path(&mut core, None, s("data"));
    add_module_path(&mut core, s("bin"), None);
    add_module_path(&mut core, None, None);
    add_name(&mut core, NameList::Safe, None);
    add_name(&mut core, NameList::Disabled, None);
    assert_eq!(core.pushed(), 0);

    add_module_path(&mut core, s("module-lists-bin"), s("module-lists-data"));
    add_name(&mut core, NameList::Safe, s("module_lists_safe"));
    add_name(&mut core, NameList::Disabled, s("module_lists_disabled"));

    add_module_path(&mut core, s(""), s(""));
    add_name(&mut core, NameList::Safe, s(""));
    add_name(&mut core, NameList::Disabled, s(""));

    assert_eq!(
        core.paths,
        [
            (b"module-lists-bin".to_vec(), b"module-lists-data".to_vec()),
            (Vec::new(), Vec::new())
        ]
    );
    assert_eq!(
        core.names(NameList::Safe),
        [b"module_lists_safe".to_vec(), Vec::new()]
    );
    assert_eq!(
        core.names(NameList::Disabled),
        [b"module_lists_disabled".to_vec(), Vec::new()]
    );
}

/// Mirrors `add_null_id`: a NULL id is pushed as a NULL entry
/// (characterized, not endorsed).
#[test]
fn add_null_id() {
    let mut core = Model::started();
    let m = core.disabled_module("module_lists_stub_null", "nodata");
    for list in TYPE_LISTS {
        add_type(&mut core, Some(m), list, None);
    }
    for list in TYPE_LISTS {
        assert_eq!(core.types(m, list), [None]);
    }
}

/* ------------------------------------------------------------------ edges */

/// The heading, then each loaded module from the first in the core's
/// order, each line before its `next` is read.
#[test]
fn log_each_loaded_module_in_order() {
    let mut core = Model::started();
    let a = core.loaded_module("a");
    let b = core.loaded_module("b");
    let c = core.loaded_module("c");
    core.disabled_module("not_loaded", "nodata");

    log_loaded_modules(&mut core);
    assert_eq!(
        core.log,
        [
            Line::Heading,
            Line::Module(c),
            Line::Module(b),
            Line::Module(a)
        ]
    );
    assert_eq!(
        *core.reads.borrow(),
        [
            Read::HasCore,
            Read::FirstModule,
            Read::Next(c),
            Read::Next(b),
            Read::Next(a)
        ]
    );
    assert_eq!(MODULE_FORMAT, "    %s");
}

/// The intentional difference: with no core, `obs_log_loaded_modules()`
/// logs nothing and reads nothing past whether there is a core. The C logs
/// the heading and then reads the core's first module, which crashes.
#[test]
fn log_with_no_core_logs_nothing() {
    let mut core = Model::default();
    log_loaded_modules(&mut core);
    assert!(core.log.is_empty());
    assert_eq!(*core.reads.borrow(), [Read::HasCore]);
}

/// A module's data path and name are read only for a module, and the
/// core's config path is read whether or not there is a core
/// (characterized, not endorsed: the C reads it unchecked).
#[test]
fn path_reads() {
    let mut core = Model::default();
    let m = core.disabled_module("module_lists_stub", "module_lists_data");
    let fs = FakeFs::default();

    assert_eq!(find_file(&core, &fs, Some(m), s("present.txt")), None);
    assert_eq!(*core.reads.borrow(), [Read::DataPath(m)]);

    core.reads.borrow_mut().clear();
    assert_eq!(
        config_path(&core, Some(m), s("settings.json")).as_deref(),
        Some(&b"module_lists_stub/settings.json"[..])
    );
    let reads = core.reads.borrow().clone();
    assert_eq!(reads.len(), 2);
    assert!(reads.contains(&Read::ModuleConfigPath));
    assert!(reads.contains(&Read::ModuleName(m)));
}

/// With a core and a NULL data path, the file is looked up as a relative
/// path; with an empty file, the data path itself.
#[test]
fn find_file_follows_module_paths() {
    let mut core = Model::started();
    let fs = FakeFs {
        files: [b"present.txt".to_vec(), b"module_lists_data".to_vec()].into(),
    };
    let m = core.disabled_module("module_lists_stub", "module_lists_data");
    core.modules[m].data_path = None;
    assert_eq!(
        find_file(&core, &fs, Some(m), s("present.txt")).as_deref(),
        Some(&b"present.txt"[..])
    );
    core.modules[m].data_path = Some(b"module_lists_data".to_vec());
    assert_eq!(
        find_file(&core, &fs, Some(m), s("")).as_deref(),
        Some(&b"module_lists_data"[..])
    );
}

fn bytes() -> impl Strategy<Value = Option<Vec<u8>>> {
    proptest::option::of(proptest::collection::vec(1u8..=255, 0..8))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    /// A search path is pushed iff there is a core and both paths, as
    /// given.
    #[test]
    fn module_path_pushed_iff_core_and_both(core in any::<bool>(), bin in bytes(), data in bytes()) {
        let mut model = Model { core, ..Model::default() };
        add_module_path(&mut model, bin.as_deref(), data.as_deref());
        let expected = match (core, bin, data) {
            (true, Some(b), Some(d)) => vec![(b, d)],
            _ => Vec::new(),
        };
        prop_assert_eq!(model.paths, expected);
    }

    /// A safe or disabled name is pushed iff there is a core and a name, on
    /// that list only.
    #[test]
    fn name_pushed_iff_core_and_name(core in any::<bool>(), safe in any::<bool>(), name in bytes()) {
        let mut model = Model { core, ..Model::default() };
        let list = if safe { NameList::Safe } else { NameList::Disabled };
        add_name(&mut model, list, name.as_deref());
        let expected: Vec<Vec<u8>> = if core { name.into_iter().collect() } else { Vec::new() };
        prop_assert_eq!(model.names(list), expected.as_slice());
        prop_assert_eq!(model.pushed(), expected.len());
    }

    /// A type id is pushed iff there is a module, NULL included, on that
    /// list only, whether or not there is a core.
    #[test]
    fn type_pushed_iff_module(core in any::<bool>(), with_module in any::<bool>(), list in 0usize..4, id in bytes()) {
        let mut model = Model { core, ..Model::default() };
        let m = model.disabled_module("m", "d");
        let list = TYPE_LISTS[list];
        add_type(&mut model, with_module.then_some(m), list, id.as_deref());
        let expected: Vec<Option<Vec<u8>>> = if with_module { vec![id] } else { Vec::new() };
        prop_assert_eq!(model.types(m, list), expected.as_slice());
        prop_assert_eq!(model.pushed(), expected.len());
        prop_assert!(model.reads.borrow().is_empty());
    }

    /// Every loaded module is logged once, in the core's order, after the
    /// heading.
    #[test]
    fn log_lists_every_loaded_module(loaded in 0usize..6, disabled in 0usize..3) {
        let mut model = Model::started();
        let mut order = Vec::new();
        for i in 0..loaded {
            order.insert(0, model.loaded_module(&format!("m{i}")));
        }
        for i in 0..disabled {
            model.disabled_module(&format!("d{i}"), "nodata");
        }
        log_loaded_modules(&mut model);
        let mut expected = vec![Line::Heading];
        expected.extend(order.into_iter().map(Line::Module));
        prop_assert_eq!(model.log, expected);
    }
}
