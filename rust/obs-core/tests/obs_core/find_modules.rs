use obs_core::find_modules::{Paths, find};

#[derive(Default)]
struct Model {
    present: bool,
    paths: Vec<Vec<usize>>,
    visited: Vec<usize>,
    reported: Vec<usize>,
    append: bool,
}

impl Paths for Model {
    fn has_core(&self) -> bool {
        self.present
    }
    fn count(&self) -> usize {
        assert!(self.present);
        self.paths.len()
    }
    fn discover(&mut self, index: usize) {
        self.visited.push(index);
        self.reported.extend_from_slice(&self.paths[index]);
        if self.append && index == 0 {
            self.paths.push(vec![3]);
        }
    }
}

#[test]
fn test_find_modules_absent_core_before_and_after_shutdown() {
    let mut model = Model::default();
    find(&mut model);
    model.present = true;
    find(&mut model);
    model.present = false;
    find(&mut model);
    assert!(model.visited.is_empty());
    assert!(model.reported.is_empty());
}

#[test]
fn test_find_modules_empty_and_missing() {
    let mut model = Model {
        present: true,
        ..Model::default()
    };
    find(&mut model);
    model.paths.push(vec![]);
    find(&mut model);
    assert_eq!(model.visited, [0]);
    assert!(model.reported.is_empty());
}

#[test]
fn test_find_modules_path_order_and_per_path_callbacks() {
    let mut model = Model {
        present: true,
        paths: vec![vec![0, 1], vec![], vec![2], vec![3]],
        ..Model::default()
    };
    find(&mut model);
    assert_eq!(model.visited, [0, 1, 2, 3]);
    assert_eq!(model.reported, [0, 1, 2, 3]);
}

#[test]
fn test_find_modules_appended_path_is_visited() {
    let mut model = Model {
        present: true,
        paths: vec![vec![0], vec![1, 2]],
        append: true,
        ..Model::default()
    };
    find(&mut model);
    assert_eq!(model.visited, [0, 1, 2]);
    assert_eq!(model.reported, [0, 1, 2, 3]);
}
