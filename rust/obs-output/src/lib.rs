//! Rust ports of `libobs/obs-output-delay.c.h`, the output stream delay,
//! `libobs/obs-output-interleave.c.h`, the encoded packet interleaver,
//! `libobs/obs-output-caption-text.c.h`, the caption text wrap,
//! `libobs/obs-output-protocols.c.h`, the output protocol-list match,
//! `libobs/obs-encoder-roi.c.h`, the encoder region of interest checks and
//! scaling, `libobs/obs-output-video-checks.c.h`, the output video size,
//! conversion and capture checks, `libobs/obs-encoder-size.c.h`, the
//! encoder size and info choices, `libobs/obs-encoder-audio-start.c.h`, the
//! encoder audio start alignment and setter gates,
//! `libobs/obs-output-stop-stats.c.h`, the output stop statistics,
//! `libobs/obs-output-raw-audio.c.h`, the raw output audio admission and
//! capture hookup, `libobs/obs-output-slots.c.h`, the output encoder
//! slot rules and setter gates, `libobs/obs-encoder-gpu-gates.c.h`, the
//! GPU encode thread gates, `libobs/obs-encoder-bind.c.h`, the encoder
//! media binding and shutdown checks, `libobs/obs-encoder-lifecycle.c.h`,
//! the encoder callback and connection lifecycle decisions,
//! `libobs/obs-encoder-packet-ts.c.h`, the encoder packet timestamps,
//! `libobs/obs-output-start.c.h`, the output start and begin-capture
//! decisions, and `libobs/obs-output-stop.c.h`, the output stop and
//! end-capture decisions.
//!
//! [`delay`], [`interleave`], [`caption_text`], [`protocols`],
//! [`encoder_roi`], [`video_checks`], [`encoder_size`],
//! [`encoder_audio_start`], [`stop_stats`], [`raw_audio`], [`output_slots`], [`encoder_gpu_gates`], [`encoder_bind`], [`encoder_lifecycle`], [`encoder_packet_ts`], [`output_start`] and [`output_stop`] are the safe cores: [`delay`] decides when a held
//! packet, start or stop leaves the delay queue, and what start, stop and
//! cleanup do to the output;
//! [`interleave`] queues an output's encoded packets until every track has
//! started, starts the tracks together at timestamp 0 and sends the packets
//! in `dts` order; [`caption_text`] wraps caption text into rows of 32
//! characters and pages of 15 rows; [`protocols`] counts the segments of a
//! service output's `;`-separated protocol list that equal a protocol;
//! [`encoder_roi`] decides which regions of interest an encoder stores and
//! scales them to its scaled size; [`video_checks`] picks an output's first
//! mixer, where its size comes from and its video conversion, checks that
//! it has what it needs to begin capturing, and clamps its congestion;
//! [`encoder_size`] fills an encoder's audio info from the mix, applies the
//! scaling cap to its video info, decides whether a scaled size is refused,
//! disabled or set, and where its width, height and sample rate come from;
//! [`encoder_audio_start`] decides when an encoder's audio starts against
//! its paired video (wait, truncate, clear or start from the buffer), the
//! pts steps, and the first refusal and divisor rule of the setters;
//! [`stop_stats`] decides the numbers and lines an output logs when it
//! stops (frames output, drawn, lagged and dropped, and their percentages);
//! [`raw_audio`] decides when a raw output takes, cuts, drops or holds an
//! audio packet against its video start, when its block loop sends a block
//! and with which timestamp, which mixes it connects, and which encoded
//! callback and delay an output starts with; [`output_slots`] finds an
//! output's first video and audio encoder, decides which encoder types,
//! indexes and mixers its setters take, which slots its getters and
//! preferred size reach, and when the encoder at index 0 takes the scaled
//! size; [`encoder_gpu_gates`] decides, for the GPU encode thread, the
//! frames skipped at its start, which frames each texture encoder takes
//! through its frame rate divisor, the keyed mutex key each encoder hands
//! on, the frame timing entry, when a frame goes back to the queue and at
//! which timestamp, and P010 or NV12 encode textures; [`encoder_bind`]
//! decides an encoder's audio type refusal, the timebase it takes from its
//! video or audio, whether it gets a frame rate divisor video, and when an
//! encoder initialized for an output that never started is shut down;
//! [`encoder_lifecycle`] decides when an encoder's first callback connects
//! it and its last disconnects it, whether a callback is new, the GPU
//! encode texture cap, the encoder group's counts (the divisor LCM start,
//! ready, the reset and the destroy when none is started), and whether the
//! SEI goes in front of the first keyframe; [`encoder_packet_ts`]
//! decides each packet's timebase, its DTS in microseconds, its time on
//! the system clock from the encoder's start and first packet, its time
//! after the pause offset, and its newest timing entry; [`output_start`] decides
//! whether a start or a begin asks the service, when a delayed restart's
//! reference is dropped, which encoders an output initializes, and whether
//! a begin resets a raw output's audio and pairs the encoders;
//! [`output_stop`] decides when a stop goes through the delay, which
//! encoders or raw connections an end of capture stops and with which
//! callback, whether it deactivates the service and cleans up the delay,
//! when restart references keep the delay, when an output that never
//! began shuts its encoders down, and when the end runs inline because
//! its thread could not be created.
//! The C ABI shims live in [`ffi`]. Later units of `obs-output.c` join this
//! crate.

#![deny(unsafe_code)]

pub mod caption_text;
pub mod delay;
pub mod encoder_audio_start;
pub mod encoder_bind;
pub mod encoder_gpu_gates;
pub mod encoder_lifecycle;
pub mod encoder_packet_ts;
pub mod encoder_roi;
pub mod encoder_size;
pub mod interleave;
pub mod output_slots;
pub mod output_start;
pub mod output_stop;
pub mod protocols;
pub mod raw_audio;
pub mod service;
pub mod stop_stats;
pub mod video_checks;

#[allow(unsafe_code)]
pub mod ffi;

pub use delay::{
    Decision, DelayOutput, Entry, NS_PER_SEC, NullLog, OBS_OUTPUT_DELAY_PRESERVE,
    OBS_OUTPUT_ENCODED, SET_DELAY, Signal, active_delay_secs, pop_decision,
};
