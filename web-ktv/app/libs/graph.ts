import { KTVVolume, MyAudioGraph } from '../types/types';
import {
	generateIRFromAudioFile,
	getFileFromStaticPath,
	IRFilePath,
} from './ir';

/**
 * Pro KTV Audio Graph:
 * *
 * 																		  |<--------Filters<---|
 * 																		  |                    |
 * mic -> Filters -> micGain ---┬--- [delay] --- [echoFeedbackGain] ---┬---> [Compressor] -> dest
 * 															|                            					 |                            					 ^
 * 															|-- [Convolver]-[reverbGain]-----------|                            					 |
 * 															|                            					 |                                      |
 * 															└-----[Dry Path] ----------------------┘                                      |
 *                                                                     |
 * player -> musicGain ------------------------------------------------┘
 */
export async function generateAudioGraph(
	nodes: MyAudioGraph,
	ctx: AudioContext
) {
	// node
	nodes.insertNode('audioDestNode', ctx.destination);

	for (const name of ['micGain', 'musicGain', 'echoFeedback', 'reverbGain']) {
		nodes.insertNode(name, ctx.createGain());
	}
	for (const name of ['echoDelay']) {
		nodes.insertNode(name, ctx.createDelay());
	}
	for (const name of ['compressor']) {
		nodes.insertNode(name, ctx.createDynamicsCompressor());
	}
	for (const name of ['analyser']) {
		nodes.insertNode(name, ctx.createAnalyser());
	}
	for (const name of ['lowCutFilter', 'presenceFilter', 'echoFilter']) {
		nodes.insertNode(name, ctx.createBiquadFilter());
	}
	for (const name of ['convolver']) {
		nodes.insertNode(name, ctx.createConvolver());
	}
	for (const name of ['micAnalyser', 'outputAnalyser']) {
		nodes.insertNode(name, ctx.createAnalyser());
	}

	// static setting
	nodes.getBiquadFilterNode('lowCutFilter').type = 'highpass';
	nodes.getBiquadFilterNode('lowCutFilter').frequency.value = 200;
	nodes.getBiquadFilterNode('presenceFilter').type = 'peaking';
	nodes.getBiquadFilterNode('presenceFilter').frequency.value = 3500;
	nodes.getBiquadFilterNode('presenceFilter').Q.value = 1.2;
	nodes.getBiquadFilterNode('presenceFilter').gain.value = 4;
	nodes.getBiquadFilterNode('echoFilter').type = 'lowpass';
	nodes.getBiquadFilterNode('echoFilter').frequency.value = 3000;
	nodes.getConvolverNode('convolver').buffer = await generateIRFromAudioFile(
		ctx,
		await getFileFromStaticPath(IRFilePath)
	);

	// connection
	nodes.connectionList([
		'micSource',
		'micAnalyser', // debug
		'lowCutFilter',
		'presenceFilter',
		'micGain',
		'compressor',
	]);
	nodes.connectionList([
		'micGain',
		'echoDelay',
		'echoFeedback',
		'echoFilter',
		'echoDelay',
	]);
	nodes.connection('echoFeedback', 'compressor');
	nodes.connectionList(['micGain', 'convolver', 'reverbGain', 'compressor']);
	nodes.connection('musicGain', 'compressor');
	nodes.connectionList([
		'compressor',
		'outputAnalyser', // debug
		'audioDestNode',
	]);

	return nodes;
}

export function setGraphVolumes(
	nodes: MyAudioGraph,
	volume: KTVVolume,
	ctx: AudioContext,
	rampTime = 0.05
) {
	const now = ctx.currentTime;

	// 1. Gain 類節點
	nodes.getGainNode('micGain').gain.setTargetAtTime(volume.mic, now, rampTime);
	nodes
		.getGainNode('musicGain')
		.gain.setTargetAtTime(volume.music, now, rampTime);
	nodes
		.getGainNode('echoFeedback')
		.gain.setTargetAtTime(volume.echo, now, rampTime);
	nodes
		.getGainNode('reverbGain')
		.gain.setTargetAtTime(volume.reverb, now, rampTime);

	// 2. 時間與濾波
	nodes
		.getDelayNode('echoDelay')
		.delayTime.setTargetAtTime(volume.delay, now, rampTime);

	// 3. Compressor 深度調試
	const comp = nodes.getDynamicsCompressorNode('compressor');
	comp.threshold.setTargetAtTime(volume.threshold, now, rampTime);
	comp.ratio.setTargetAtTime(volume.ratio, now, rampTime);
	comp.knee.setTargetAtTime(volume.knee, now, rampTime);
	comp.attack.setTargetAtTime(volume.attack, now, rampTime);
	comp.release.setTargetAtTime(volume.release, now, rampTime);
}

export async function getAudioMedia() {
	return await navigator.mediaDevices.getUserMedia({
		audio: {
			sampleRate: { ideal: 48000 },
			echoCancellation: true,
			autoGainControl: false,
			noiseSuppression: { ideal: true },
			channelCount: 1,
		},
	});
}

export function getAudioContext() {
	const Context = window.AudioContext;
	return new Context();
}
