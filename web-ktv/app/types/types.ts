export interface KTVNode {
	stream?: MediaStream;
	micSource?: MediaStreamAudioSourceNode;
	micGain?: GainNode;
	musicGain?: GainNode;
	echoDelay?: DelayNode;
	echoFeedback?: GainNode;
	lowCutFilter?: BiquadFilterNode;
	presenceFilter?: BiquadFilterNode;
	echoFilter?: BiquadFilterNode;
	compressor?: DynamicsCompressorNode;
	analyser?: AnalyserNode;
}

export interface KTVVolume {
	mic: number;
	music: number;
	echo: number; // 迴響強度 (Feedback)
	delay: number; // 迴響延遲時間 (秒)
	ratio: number; // 壓縮比例
	ducking: number; // 門檻值 (Threshold)
}
