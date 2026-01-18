pub mod biquad;

// 定義一個處理立體聲幀的介面
// 嵌入式思維：我們處理的是 Frame (L, R)，而不是由 Vec 包裹的任意長度 Buffer
// 這樣可以保證數據在暫存器中的連續性 (Register Locality)
pub trait AudioProcessor: Send + Sync {
    // 初始化內部狀態 (例如分配 Delay Line 記憶體)
    // 這一步只在系統啟動時呼叫，允許 allocation
    fn prepare(&mut self, sample_rate: f32);

    // 實時處理函數 (Hot Path)
    // 嚴禁 allocation, 嚴禁 Mutex lock
    fn process(&mut self, left: &mut f32, right: &mut f32);
}

// 實作一個簡單的增益處理器 (Volume Control) 作為範例
pub struct GainProcessor {
    pub gain_db: f32,
    linear_gain: f32,
}

impl GainProcessor {
    pub fn new(gain_db: f32) -> Self {
        Self {
            gain_db,
            linear_gain: 10.0f32.powf(gain_db / 20.0),
        }
    }

    // 用於 UI 更新參數 (非實時線程呼叫)
    pub fn set_gain(&mut self, db: f32) {
        self.gain_db = db;
        self.linear_gain = 10.0f32.powf(db / 20.0);
    }
}

impl AudioProcessor for GainProcessor {
    fn prepare(&mut self, _sample_rate: f32) {}

    #[inline(always)] // 強制內聯，減少函數跳轉開銷
    fn process(&mut self, left: &mut f32, right: &mut f32) {
        *left *= self.linear_gain;
        *right *= self.linear_gain;
    }
}
