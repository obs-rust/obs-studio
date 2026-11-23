use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct Error(pub(crate) &'static str);

pub struct Engine;

impl Engine {
    pub fn open(
        path: &Path,
        class_id: &str,
        sample_rate: f64,
        max_frames: i32,
        arrangement: u64,
    ) -> Result<Self, Error> {
        let _ = (path, class_id, sample_rate, max_frames, arrangement);
        todo!()
    }

    pub fn input_channel(&mut self, bus: usize, channel: usize) -> Option<&mut [f32]> {
        let _ = (bus, channel);
        todo!()
    }

    pub fn output_channel(&self, bus: usize, channel: usize) -> Option<&[f32]> {
        let _ = (bus, channel);
        todo!()
    }

    pub fn activate(&mut self, active: bool) -> Result<(), Error> {
        let _ = active;
        todo!()
    }

    pub fn set_processing(&mut self, active: bool) -> Result<(), Error> {
        let _ = active;
        todo!()
    }

    pub fn process(&mut self, frames: usize, system_time: i64) -> Result<(), Error> {
        let _ = (frames, system_time);
        todo!()
    }

    pub fn restore_state(&mut self, component: &[u8], controller: &[u8]) -> Result<(), Error> {
        let _ = (component, controller);
        todo!()
    }

    pub fn save_state(&self) -> Result<(Vec<u8>, Vec<u8>), Error> {
        todo!()
    }
}
