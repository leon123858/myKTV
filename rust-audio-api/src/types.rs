pub const AUDIO_UNIT_SIZE: usize = 64;

/// 代表音訊圖中一次 pull 操作所處理的最小單位 (64 個 frame，每個 frame 是 f32 雙聲道)
pub type AudioUnit = [[f32; 2]; AUDIO_UNIT_SIZE];

/// 建立一個靜音 (全為 0) 的 AudioUnit
pub fn empty_audio_unit() -> AudioUnit {
    [[0.0; 2]; AUDIO_UNIT_SIZE]
}
