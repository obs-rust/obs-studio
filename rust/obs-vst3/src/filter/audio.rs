//! Planar packet assembly at the OBS filter boundary.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const DISCONTINUITY_NS: u64 = 1_000_000_000;
const AUDIO_ALIGNMENT: usize = 32;
const ALIGN_SAMPLES: usize = AUDIO_ALIGNMENT / std::mem::size_of::<f32>();

pub struct Packet<'a> {
    pub frames: usize,
    pub timestamp: u64,
    pub stride: usize,
    pub samples: &'a mut [f32],
}

#[derive(Clone)]
pub struct AuxCapture(Arc<Mutex<AuxQueue>>);

struct AuxQueue {
    planes: Vec<VecDeque<f32>>,
    last_timestamp: u64,
}

impl AuxCapture {
    pub fn push(&self, timestamp: u64, planes: &[&[f32]]) {
        let Ok(mut queue) = self.0.lock() else { return };
        if discontinuous(queue.last_timestamp, timestamp) {
            clear_planes(&mut queue.planes);
        }
        queue.last_timestamp = timestamp;
        append(&mut queue.planes, planes);
    }

    pub fn clear(&self) {
        if let Ok(mut queue) = self.0.lock() {
            clear_planes(&mut queue.planes);
        }
    }

    pub fn reset_timestamp(&self) {
        if let Ok(mut queue) = self.0.lock() {
            queue.last_timestamp = 0;
        }
    }

    fn take_block(&self, planes: &mut [Vec<f32>]) {
        let Ok(mut queue) = self.0.lock() else {
            planes.iter_mut().for_each(|plane| plane.fill(0.0));
            return;
        };
        for (src, dst) in queue.planes.iter_mut().zip(planes) {
            let frames = dst.len();
            if src.len() < frames {
                // The reference appends a full block, keeping the tail queued.
                src.resize(src.len() + frames, 0.0);
            }
            dst.iter_mut()
                .zip(src.drain(..frames))
                .for_each(|(to, from)| *to = from);
        }
    }
}

pub struct FilterAudio {
    input: Vec<VecDeque<f32>>,
    output: Vec<VecDeque<f32>>,
    aux: AuxCapture,
    pending: VecDeque<(usize, u64)>,
    main_block: Vec<Vec<f32>>,
    aux_block: Vec<Vec<f32>>,
    packet: Vec<f32>,
    last_timestamp: u64,
    block_frames: usize,
    latency: u64,
}

impl FilterAudio {
    pub fn new(channels: usize, block_frames: usize, latency: u64) -> Self {
        assert!(channels > 0 && block_frames > 0);
        let reserve = block_frames * 8;
        // VecDeque::clone need not preserve spare capacity.
        let queues = || {
            (0..channels)
                .map(|_| VecDeque::with_capacity(reserve))
                .collect()
        };
        let block = || vec![vec![0.0; block_frames]; channels];
        Self {
            input: queues(),
            output: queues(),
            aux: AuxCapture(Arc::new(Mutex::new(AuxQueue {
                planes: queues(),
                last_timestamp: 0,
            }))),
            pending: VecDeque::with_capacity(reserve),
            main_block: block(),
            aux_block: block(),
            packet: Vec::with_capacity(reserve * channels + ALIGN_SAMPLES - 1),
            last_timestamp: 0,
            block_frames,
            latency,
        }
    }

    pub fn aux_capture(&self) -> AuxCapture {
        self.aux.clone()
    }

    pub fn capture_aux(&self, timestamp: u64, planes: &[&[f32]]) {
        self.aux.push(timestamp, planes);
    }

    pub fn disconnect_aux(&self) {
        self.aux.clear();
    }

    pub fn filter(
        &mut self,
        timestamp: u64,
        planes: &[&[f32]],
        aux_enabled: bool,
        mut process: impl FnMut(&mut [Vec<f32>], Option<&[Vec<f32>]>),
    ) -> Option<Packet<'_>> {
        let frames = planes.first()?.len();
        if planes.len() != self.input.len() || planes.iter().any(|plane| plane.len() != frames) {
            return None;
        }
        if discontinuous(self.last_timestamp, timestamp) {
            clear_planes(&mut self.input);
            clear_planes(&mut self.output);
            self.pending.clear();
        }
        self.last_timestamp = timestamp;
        self.pending.push_back((frames, timestamp));
        append(&mut self.input, planes);

        while self.input[0].len() >= self.block_frames {
            for (src, dst) in self.input.iter_mut().zip(&mut self.main_block) {
                dst.iter_mut()
                    .zip(src.drain(..self.block_frames))
                    .for_each(|(to, from)| *to = from);
            }
            if aux_enabled {
                self.aux.take_block(&mut self.aux_block);
            }
            process(
                &mut self.main_block,
                aux_enabled.then_some(self.aux_block.as_slice()),
            );
            for (dst, src) in self.output.iter_mut().zip(&self.main_block) {
                dst.extend(src);
            }
        }

        let &(frames, timestamp) = self.pending.front()?;
        if self.output[0].len() < frames {
            return None;
        }
        self.pending.pop_front();
        let stride = frames.next_multiple_of(ALIGN_SAMPLES);
        let samples = stride * self.output.len();
        // A Vec<f32> only promises f32 alignment. Reserve space to align
        // the actual OBS-visible slice, including after buffer growth.
        self.packet.resize(samples + ALIGN_SAMPLES - 1, 0.0);
        let offset = self.packet.as_ptr().align_offset(AUDIO_ALIGNMENT);
        let packet = &mut self.packet[offset..offset + samples];
        if stride > 0 {
            for (src, plane) in self.output.iter_mut().zip(packet.chunks_exact_mut(stride)) {
                for (to, from) in plane.iter_mut().zip(src.drain(..frames)) {
                    *to = from;
                }
            }
        }
        Some(Packet {
            frames,
            timestamp: timestamp.wrapping_sub(self.latency),
            stride,
            samples: packet,
        })
    }
}

fn discontinuous(last: u64, next: u64) -> bool {
    last != 0 && last.abs_diff(next) > DISCONTINUITY_NS
}

fn append(destination: &mut [VecDeque<f32>], source: &[&[f32]]) {
    for (to, from) in destination.iter_mut().zip(source) {
        to.extend(*from);
    }
}

fn clear_planes(planes: &mut [VecDeque<f32>]) {
    planes.iter_mut().for_each(VecDeque::clear);
}
