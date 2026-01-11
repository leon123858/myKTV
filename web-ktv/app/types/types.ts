export class MyAudioGraph {
	public nodes: { [key: string]: AudioNode | undefined } = {};
	public streamList: { [key: string]: MediaStream | undefined } = {};

	constructor() {}

	insertStream(name: string, stream: MediaStream) {
		this.streamList[name] = stream;
	}

	insertNode(name: string, node: AudioNode) {
		this.nodes[name] = node;
	}

	getStream(name: string) {
		return this.streamList[name];
	}

	getNode(name: string) {
		return this.nodes[name];
	}

	getGainNode(name: string) {
		return this.nodes[name] as GainNode;
	}
	getMediaStreamAudioSourceNode(name: string) {
		return this.nodes[name] as MediaStreamAudioSourceNode;
	}
	getDelayNode(name: string) {
		return this.nodes[name] as DelayNode;
	}
	getBiquadFilterNode(name: string) {
		return this.nodes[name] as BiquadFilterNode;
	}
	getDynamicsCompressorNode(name: string) {
		return this.nodes[name] as DynamicsCompressorNode;
	}
	getAnalyserNode(name: string) {
		return this.nodes[name] as AnalyserNode;
	}
	getAudioBufferSourceNode(name: string) {
		return this.nodes[name] as AudioBufferSourceNode;
	}
	getConvolverNode(name: string) {
		return this.nodes[name] as ConvolverNode;
	}

	connection(name1: string, name2: string) {
		if (this.nodes[name1] && this.nodes[name2]) {
			this.nodes[name1].connect(this.nodes[name2]);
		} else {
			throw 'Node not found';
		}
	}

	connectionList(names: string[]) {
		names.reduce((pre, cur) => {
			if (pre != '') {
				this.connection(pre, cur);
			}
			return cur;
		}, '');
	}

	stopAll() {
		for (const stream in this.streamList) {
			if (this.streamList[stream]) {
				this.streamList[stream].getTracks().forEach((track) => track.stop());
			}
		}
	}
}

export interface KTVVolume {
	// Gain 相關
	mic: number;
	music: number;
	echo: number; // Feedback Gain
	reverb: number; // Reverb Send Gain

	// 時間相關
	delay: number; // Echo Delay Time

	// Compressor 核心參數
	threshold: number;
	ratio: number;
	knee: number;
	attack: number;
	release: number;
}
