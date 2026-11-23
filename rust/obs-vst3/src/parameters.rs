#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub offset: i32,
    pub value: f64,
}

#[derive(Debug, Default)]
pub struct Queue;

impl Queue {
    pub fn points(&self) -> &[Point] {
        todo!()
    }

    pub fn insert(&mut self, point: Point) -> usize {
        let _ = point;
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Restart {
    pub latency_changed: bool,
    pub values_changed: bool,
}

impl Restart {
    pub fn decode(flags: i32) -> Option<Self> {
        let _ = flags;
        todo!()
    }
}
