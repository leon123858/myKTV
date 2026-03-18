pub const AUDIO_UNIT_SIZE: usize = 64;

/// Represents the minimum unit processed per pull in the audio graph (64 frames, each a stereo f32)
pub type AudioUnit = [[f32; 2]; AUDIO_UNIT_SIZE];

/// Creates a silent (all zeros) AudioUnit
pub fn empty_audio_unit() -> AudioUnit {
    [[0.0; 2]; AUDIO_UNIT_SIZE]
}
