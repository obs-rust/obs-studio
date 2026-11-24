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

#[cfg(test)]
mod tests {
    use super::*;

    fn passthrough(_: &mut [Vec<f32>], _: Option<&[Vec<f32>]>) {}

    #[test]
    fn returns_oldest_packet_and_preserves_planar_stride() {
        let mut filter = FilterAudio::new(2, 4, 5);
        assert!(
            filter
                .filter(10, &[&[1., 2., 3.], &[11., 12., 13.]], false, passthrough)
                .is_none()
        );
        let packet = filter
            .filter(
                20,
                &[&[4., 5., 6., 7., 8.], &[14., 15., 16., 17., 18.]],
                false,
                passthrough,
            )
            .unwrap();
        assert_eq!((packet.frames, packet.timestamp, packet.stride), (3, 5, 8));
        assert_eq!(&packet.samples[..3], &[1., 2., 3.]);
        assert_eq!(&packet.samples[8..11], &[11., 12., 13.]);
        let packet = filter
            .filter(30, &[&[9.], &[19.]], false, passthrough)
            .unwrap();
        assert_eq!((packet.frames, packet.timestamp), (5, 15));
        assert_eq!(&packet.samples[..5], &[4., 5., 6., 7., 8.]);
    }

    #[test]
    fn timestamp_gap_resets_only_above_one_second() {
        for gap in [999_999_999, 1_000_000_000, 1_000_000_001] {
            let mut filter = FilterAudio::new(1, 4, 5);
            assert!(
                filter
                    .filter(100, &[&[1., 2.]], false, passthrough)
                    .is_none()
            );
            let result = filter.filter(100 + gap, &[&[3., 4.]], false, passthrough);
            if gap > 1_000_000_000 {
                assert!(result.is_none());
                let packet = filter
                    .filter(101 + gap, &[&[5., 6.]], false, passthrough)
                    .unwrap();
                assert_eq!(packet.timestamp, 95 + gap);
                assert_eq!(&packet.samples[..2], &[3., 4.]);
            } else {
                let packet = result.unwrap();
                assert_eq!(packet.timestamp, 95);
                assert_eq!(&packet.samples[..2], &[1., 2.]);
            }
        }
    }

    #[test]
    fn short_sidechain_appends_full_silent_block_and_disconnect_clears_it() {
        let mut filter = FilterAudio::new(1, 4, 0);
        filter.capture_aux(10, &[&[1., 2.]]);
        let aux_copy = |main: &mut [Vec<f32>], aux: Option<&[Vec<f32>]>| {
            if let Some(aux) = aux {
                main[0].copy_from_slice(&aux[0]);
            }
        };
        let packet = filter.filter(20, &[&[9.; 4]], true, aux_copy).unwrap();
        assert_eq!(&packet.samples[..4], &[1., 2., 0., 0.]);
        filter.capture_aux(21, &[&[3.; 4]]);
        let packet = filter.filter(22, &[&[9.; 4]], true, aux_copy).unwrap();
        assert_eq!(&packet.samples[..4], &[0., 0., 3., 3.]);
        filter.disconnect_aux();
        let packet = filter.filter(23, &[&[9.; 4]], true, aux_copy).unwrap();
        assert_eq!(&packet.samples[..4], &[0.; 4]);
    }
}

#[cfg(test)]
mod alignment_tests {
    use super::*;

    #[test]
    fn output_storage_is_aligned_for_obs_meter() {
        let mut filters: Vec<_> = (1..32)
            .map(|frames| FilterAudio::new(2, frames, 0))
            .collect();
        for (frames, filter) in (1..32).zip(&mut filters) {
            let input = vec![0.0; frames];
            let packet = filter
                .filter(1, &[&input, &input], false, |_, _| {})
                .unwrap();
            assert_eq!(packet.samples.as_ptr().align_offset(32), 0);
            assert_eq!(packet.samples[packet.stride..].as_ptr().align_offset(32), 0);
        }
    }

    #[test]
    fn generated_packets_match_native_filter_transcript() {
        let mut filter = FilterAudio::new(2, 4, 5);
        let mut seed = 0x5537_u32;
        let mut timestamp = 2_000_000_000_u64;
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        let mut record = |value: u64| digest = (digest ^ value).wrapping_mul(0x0100_0000_01b3);
        for index in 0..2048 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            timestamp += 1 + u64::from((seed >> 16) % 64);
            if index % 37 == 0 {
                timestamp += 1_000_000_000;
            }
            if index % 61 == 0 {
                timestamp -= 1_500_000_000;
            }
            let frames = 1 + (seed >> 3) % 17;
            let aux_frames = (seed >> 9) % 13;
            let enabled = seed & 3 != 0;
            if index % 23 == 0 {
                filter.disconnect_aux();
            }
            let main: [_; 2] = std::array::from_fn(|channel| {
                (0..frames)
                    .map(|i| ((timestamp + 17 * channel as u64 + u64::from(i)) % 256) as f32)
                    .collect::<Vec<_>>()
            });
            let aux: [_; 2] = std::array::from_fn(|channel| {
                (0..aux_frames)
                    .map(|i| ((7 * timestamp + 43 * channel as u64 + u64::from(i)) % 32) as f32)
                    .collect::<Vec<_>>()
            });
            if aux_frames != 0 && enabled {
                filter.capture_aux(timestamp, &[&aux[0], &aux[1]]);
            }
            let packet = filter.filter(timestamp, &[&main[0], &main[1]], enabled, |main, aux| {
                for (channel, plane) in main.iter_mut().enumerate() {
                    for (index, sample) in plane.iter_mut().enumerate() {
                        *sample += aux.map_or(0.0, |aux| aux[channel][index]);
                    }
                }
            });
            if let Some(packet) = packet {
                record(packet.frames as u64);
                record(packet.timestamp);
                for channel in 0..2 {
                    let start = channel * packet.stride;
                    for sample in &packet.samples[start..start + packet.frames] {
                        record(u64::from(sample.to_bits()));
                    }
                }
            } else {
                record(0);
            }
        }
        // Measured against the C++ callback at 16bc737b4 with a deterministic
        // main-plus-aux processor and the same packet sequence.
        assert_eq!(digest, 0x9992_1b9e_fe30_9e4a);
    }
}
