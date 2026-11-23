#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekOrigin {
    Start,
    Current,
    End,
}

#[derive(Debug, Default)]
pub struct StateStream {
    bytes: Vec<u8>,
    cursor: usize,
}

impl StateStream {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub fn read(&mut self, output: &mut [u8]) -> usize {
        let start = self.cursor.min(self.bytes.len());
        let length = output.len().min(self.bytes.len() - start);
        output[..length].copy_from_slice(&self.bytes[start..start + length]);
        self.cursor = start + length;
        length
    }

    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let end = self
            .cursor
            .checked_add(bytes.len())
            .filter(|end| i64::try_from(*end).is_ok())
            .ok_or(ErrorKind::InvalidInput)?;
        if end > self.bytes.len() {
            self.bytes
                .try_reserve(end - self.bytes.len())
                .map_err(io::Error::other)?;
            self.bytes.resize(end, 0);
        }
        self.bytes[self.cursor..end].copy_from_slice(bytes);
        self.cursor = end;
        Ok(bytes.len())
    }

    pub fn seek(&mut self, offset: i64, origin: SeekOrigin) -> std::io::Result<i64> {
        let base = match origin {
            SeekOrigin::Start => 0,
            SeekOrigin::Current => self.position(),
            SeekOrigin::End => self.bytes.len() as i64,
        };
        let position = base.checked_add(offset).ok_or(ErrorKind::InvalidInput)?;
        self.cursor = usize::try_from(position).map_err(|_| ErrorKind::InvalidInput)?;
        Ok(position)
    }

    pub fn position(&self) -> i64 {
        self.cursor as i64
    }

    pub fn saved_bytes(&self) -> &[u8] {
        &self.bytes[..self.cursor.min(self.bytes.len())]
    }
}

use std::io::{self, ErrorKind};
