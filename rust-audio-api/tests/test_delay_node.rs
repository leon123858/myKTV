use rust_audio_api::nodes::DelayNode;
use rust_audio_api::types::{AUDIO_UNIT_SIZE, empty_audio_unit};

#[test]
fn test_delay_node_initialization() {
    let delay_node = DelayNode::new(10, 5);
    let _ = delay_node;
}

#[test]
fn test_delay_node_delay_behavior() {
    let mut delay_node = DelayNode::new(10, 2);
    let mut output = empty_audio_unit();
    let mut input1 = empty_audio_unit();
    let mut input2 = empty_audio_unit();

    // 構造兩個獨特的 input 以供追蹤
    for i in 0..AUDIO_UNIT_SIZE {
        input1[i] = [1.0, 1.0];
        input2[i] = [2.0, 2.0];
    }

    // 第 1 次 process: push input1, pop 初始化的靜音 (delay buffer 有 2 個靜音)
    delay_node.process(Some(&input1), &mut output);
    assert_eq!(output, empty_audio_unit());

    // 第 2 次 process: push input2, pop 另一個初始化的靜音
    delay_node.process(Some(&input2), &mut output);
    assert_eq!(output, empty_audio_unit());

    // 第 3 次 process: push 靜音, pop 應該是 input1 (之前最早放入的)
    delay_node.process(None, &mut output);
    assert_eq!(output, input1);

    // 第 4 次 process: push 靜音, pop 應該是 input2
    delay_node.process(None, &mut output);
    assert_eq!(output, input2);
}

#[test]
fn test_delay_node_set_delay_units_increase() {
    let mut delay_node = DelayNode::new(10, 1);
    let mut output = empty_audio_unit();
    let mut input1 = empty_audio_unit();
    input1[0] = [1.0, 1.0];

    // 先 push input1
    delay_node.process(Some(&input1), &mut output);
    assert_eq!(output, empty_audio_unit());

    // 把 delay 提升到 3 (原來是 1)。
    // 因為提升了 2 單位的 delay，會在 queue 前面補 2 個靜音，原來被 push 的 input1 也會被往後推
    delay_node.set_delay_units(3);

    // push 靜音, pop -> 補進去的第 1 個靜音
    delay_node.process(None, &mut output);
    assert_eq!(output, empty_audio_unit());

    // push 靜音, pop -> 補進去的第 2 個靜音
    delay_node.process(None, &mut output);
    assert_eq!(output, empty_audio_unit());

    // push 靜音, pop -> 應該是之前的 input1 了
    delay_node.process(None, &mut output);
    assert_eq!(output, input1);
}

#[test]
fn test_delay_node_set_delay_units_decrease() {
    let mut delay_node = DelayNode::new(10, 2);
    let mut output = empty_audio_unit();
    let mut input1 = empty_audio_unit();
    input1[0] = [1.0, 1.0];

    // push input1 (隊列目前: [靜音, 靜音, input1])
    delay_node.process(Some(&input1), &mut output);
    assert_eq!(output, empty_audio_unit()); // pop 掉一個靜音，隊列剩: [靜音, input1]

    // 把 delay 降為 0，這會把 queue 裡面老的資料丟棄 (原定 2 現在 0 -> 丟掉最前面的)
    // 但剛才 `set_delay_units` 的實作是：減少 delay 就是直接丟掉 queue 前面的 elements
    // 當時的 self.delay_units 還是 2。但是現在如果設為 0，會變成減去 2 個 elements (也就是把 靜音 和 input1 全丟了，這樣有問題，因為 queue 這時可能長度不一。等等，其實 queue 的長度應該永遠維持在 delay_units+1 左右。)
    // 我們可以從測試來驗證看看。
    // 如果把 delay 從 2 縮減到 0，會 pop 掉 2 個，也就是隊列裡的 [靜音, input1] 都會被丟棄，隊列變空。
    // 如果再 push 一個靜音，pop 就只會有剛 push 的靜音，所以 input1 就被切掉了（跳躍播放）。
    // 在真實 delay time 修改中這也是正常的作法之一（直接跳轉，雖然可能有 pop 音）。

    delay_node.set_delay_units(0);

    let mut input2 = empty_audio_unit();
    input2[0] = [2.0, 2.0];

    // 再次 push input2, pop 時隊列裡只有 input2，因為 delay 是 0
    delay_node.process(Some(&input2), &mut output);
    assert_eq!(output, input2);
}
