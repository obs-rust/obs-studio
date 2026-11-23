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
        let main_input = input
            .iter()
            .position(|bus| bus.kind == Kind::Main)
            .ok_or(Error::MissingMainInput)?;
        let main_output = output
            .iter()
            .position(|bus| bus.kind == Kind::Main)
            .ok_or(Error::MissingMainOutput)?;
        let sidechain = input
            .iter()
            .position(|bus| bus.kind == Kind::Auxiliary && matches!(bus.channels, 1 | 2));
        let input = select(input, |index| {
            if index == main_input {
                Some(main_arrangement)
            } else if Some(index) == sidechain {
                Some(if input[index].channels == 1 {
                    1 << 19
                } else {
                    3
                })
            } else {
                None
            }
        })?;
        let output = select(output, |index| {
            (index == main_output).then_some(main_arrangement)
        })?;
        Ok(Self {
            input,
            output,
            main_input,
            main_output,
            sidechain,
        })
    }

    pub fn matches_active_arrangements(
        &self,
        input: &[Description],
        output: &[Description],
    ) -> bool {
        fn matches(selection: &[Selection], buses: &[Description]) -> bool {
            selection.len() == buses.len()
                && selection.iter().zip(buses).all(|(selected, bus)| {
                    !selected.enabled || bus.arrangement == Some(selected.arrangement)
                })
        }
        matches(&self.input, input) && matches(&self.output, output)
    }
}

fn select(
    buses: &[Description],
    desired_arrangement: impl Fn(usize) -> Option<u64>,
) -> Result<Vec<Selection>, Error> {
    buses
        .iter()
        .enumerate()
        .map(|(index, bus)| {
            let selected = desired_arrangement(index);
            let arrangement = selected
                .or(bus.arrangement)
                .ok_or(Error::ArrangementUnavailable)?;
            Ok(Selection {
                enabled: selected.is_some(),
                arrangement,
            })
        })
        .collect()
}
