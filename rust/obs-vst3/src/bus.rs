#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Main,
    Auxiliary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Description {
    pub kind: Kind,
    pub channels: usize,
    pub arrangement: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub enabled: bool,
    pub arrangement: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BusPlan {
    pub input: Vec<Selection>,
    pub output: Vec<Selection>,
    pub main_input: usize,
    pub main_output: usize,
    pub sidechain: Option<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    MissingMainInput,
    MissingMainOutput,
    ArrangementUnavailable,
}

impl BusPlan {
    pub fn select(
        input: &[Description],
        output: &[Description],
        main_arrangement: u64,
    ) -> Result<Self, Error> {
        let _ = (input, output, main_arrangement);
        todo!()
    }

    pub fn matches_active_arrangements(
        &self,
        input: &[Description],
        output: &[Description],
    ) -> bool {
        let _ = (input, output);
        todo!()
    }
}
