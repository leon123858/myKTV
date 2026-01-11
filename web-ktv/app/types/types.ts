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
	mic: number;
	music: number;
	echo: number; // 迴響強度 (Feedback)
	delay: number; // 迴響延遲時間 (秒)
	ratio: number; // 壓縮比例
	ducking: number; // 門檻值 (Threshold)
}
