/// 代表音訊圖中的一個節點
pub trait AudioNode: Send {
    /// 取得下一個音訊 Frame (假設我們預設處理雙聲道 Stereo，以 f32 為主)
    /// 這裡簡單起見，我們回傳一個 [f32; 2] 代表左右聲道
    fn process(&mut self) -> [f32; 2];
}
