//! Exact pinned VST3 SDK layouts on the supported 64-bit targets.
use std::mem::{align_of, offset_of, size_of};
use vst3::Steinberg::TUID;
use vst3::Steinberg::Vst::{
    AudioBusBuffers, BusInfo, ParameterInfo, ProcessContext, ProcessData, ProcessSetup,
};

macro_rules! layout {
    ($ty:ty, $size:literal, $align:literal; $($field:ident = $offset:literal),* $(,)?) => {
        const _: () = {
            assert!(size_of::<$ty>() == $size);
            assert!(align_of::<$ty>() == $align);
            $(assert!(offset_of!($ty, $field) == $offset);)*
        };
    };
}

layout!(TUID, 16, 1;);
layout!(AudioBusBuffers, 24, 8;
    numChannels = 0,
    silenceFlags = 8,
    __field0 = 16,
);
layout!(BusInfo, 276, 4;
    mediaType = 0,
    direction = 4,
    channelCount = 8,
    name = 12,
    busType = 268,
    flags = 272,
);
layout!(ParameterInfo, 792, 8;
    id = 0,
    title = 4,
    shortTitle = 260,
    units = 516,
    stepCount = 772,
    defaultNormalizedValue = 776,
    unitId = 784,
    flags = 788,
);
layout!(ProcessSetup, 24, 8;
    processMode = 0,
    symbolicSampleSize = 4,
    maxSamplesPerBlock = 8,
    sampleRate = 16,
);
layout!(ProcessData, 80, 8;
    processMode = 0,
    symbolicSampleSize = 4,
    numSamples = 8,
    numInputs = 12,
    numOutputs = 16,
    inputs = 24,
    outputs = 32,
    inputParameterChanges = 40,
    outputParameterChanges = 48,
    inputEvents = 56,
    outputEvents = 64,
    processContext = 72,
);
layout!(ProcessContext, 112, 8;
    state = 0,
    sampleRate = 8,
    projectTimeSamples = 16,
    systemTime = 24,
    continousTimeSamples = 32,
    projectTimeMusic = 40,
    barPositionMusic = 48,
    cycleStartMusic = 56,
    cycleEndMusic = 64,
    tempo = 72,
    timeSigNumerator = 80,
    timeSigDenominator = 84,
    chord = 88,
    smpteOffsetSubframes = 92,
    frameRate = 96,
    samplesToNextClock = 104,
);
