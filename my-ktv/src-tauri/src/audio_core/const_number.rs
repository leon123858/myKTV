// 系統採樣率標準：KTV 系統強烈建議鎖定 48kHz
pub const PREFERRED_SAMPLE_RATE: u32 = 48000;
// 延遲目標：128 samples @ 48kHz ~= 2.6ms (單向)
pub const PREFERRED_BUFFER_SIZE: u32 = 128;
// 定義 RingBuffer 容量：例如 1 秒的緩衝 (48000 * 2 channels)
pub const RING_BUFFER_CAPACITY: usize = 65536;
