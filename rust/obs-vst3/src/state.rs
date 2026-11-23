#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekOrigin {
    Start,
    Current,
    End,
}

#[derive(Debug, Default)]
pub struct StateStream;

impl StateStream {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let _ = bytes;
        todo!()
    }

    pub fn read(&mut self, output: &mut [u8]) -> usize {
        let _ = output;
        todo!()
    }

    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let _ = bytes;
        todo!()
    }

    pub fn seek(&mut self, offset: i64, origin: SeekOrigin) -> std::io::Result<i64> {
        let _ = (offset, origin);
        todo!()
    }

    pub fn position(&self) -> i64 {
        todo!()
    }

    pub fn saved_bytes(&self) -> &[u8] {
        todo!()
    }
}
