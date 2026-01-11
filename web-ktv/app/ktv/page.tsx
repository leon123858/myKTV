'use client';

import { useState, useRef, useEffect, useCallback } from 'react';
import { Mic, Music, Play, Pause, RotateCcw } from 'lucide-react';
import { KTVVolume, MyAudioGraph } from '../types/types';
import ControlSlider from '../components/controlSide';
import { generateFakeIRBuffer } from '../libs/ir';

export default function KTVPage() {
	const [isEngineRunning, setIsEngineRunning] = useState(false);
	const [isPlaying, setIsPlaying] = useState(false);
	const [volume, setVolumeState] = useState<KTVVolume>({
		mic: 0.8,
		music: 0.6,
		echo: 0.3,
		reverb: 0.4,
		delay: 0.2,
		threshold: -40,
		ratio: 14,
		knee: 30,
		attack: 0.003,
		release: 0.25,
	});

	// 音樂播放相關狀態
	const [audioBuffer, setAudioBuffer] = useState<AudioBuffer | null>(null);
	const [startTime, setStartTime] = useState(0); // 紀錄開始播放的絕對時間
	const [pausedAt, setPausedAt] = useState(0); // 紀錄暫停時已播放了幾秒
	const [isResetting, setIsResetting] = useState(false);

	const audioCtx = useRef<AudioContext | null>(null);
	const nodes = useRef<MyAudioGraph>(new MyAudioGraph());

	const applyVolumeSettings = useCallback(
		(rampTime = 0.05) => {
			const ctx = audioCtx.current;
			const n = nodes.current;
			if (!ctx || !n) return;

			const v = volume;
			const now = ctx.currentTime;

			// 1. Gain 類節點
			n.getGainNode('micGain').gain.setTargetAtTime(v.mic, now, rampTime);
			n.getGainNode('musicGain').gain.setTargetAtTime(v.music, now, rampTime);
			n.getGainNode('echoFeedback').gain.setTargetAtTime(v.echo, now, rampTime);
			n.getGainNode('reverbGain').gain.setTargetAtTime(v.reverb, now, rampTime);

			// 2. 時間與濾波
			n.getDelayNode('echoDelay').delayTime.setTargetAtTime(
				v.delay,
				now,
				rampTime
			);

			// 3. Compressor 深度調試
			const comp = n.getDynamicsCompressorNode('compressor');
			comp.threshold.setTargetAtTime(v.threshold, now, rampTime);
			comp.ratio.setTargetAtTime(v.ratio, now, rampTime);
			comp.knee.setTargetAtTime(v.knee, now, rampTime);
			comp.attack.setTargetAtTime(v.attack, now, rampTime);
			comp.release.setTargetAtTime(v.release, now, rampTime);
		},
		[volume]
	);

	useEffect(() => {
		if (isEngineRunning) {
			applyVolumeSettings();
		}
	}, [volume, isEngineRunning, applyVolumeSettings]);

	const handleReset = () => {
		nodes.current.getAudioBufferSourceNode('musicSource').stop();
		setPausedAt(0);
		setIsPlaying(false);
		setIsResetting(true);
		setTimeout(() => setIsResetting(false), 200);
	};

	const initAudio = async () => {
		if (audioCtx.current) return;

		try {
			const Context = window.AudioContext;
			audioCtx.current = new Context();
			const ctx = audioCtx.current;

			const stream = await navigator.mediaDevices.getUserMedia({
				audio: {
					sampleRate: { ideal: 48000 },
					echoCancellation: true,
					autoGainControl: false,
					noiseSuppression: { ideal: true },
					channelCount: 1,
				},
			});
			nodes.current.insertStream('mic', stream);

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

			// node
			nodes.current.insertNode(
				'micSource',
				ctx.createMediaStreamSource(stream)
			);
			nodes.current.insertNode('audioDestNode', ctx.destination);

			for (const name of [
				'micGain',
				'musicGain',
				'echoFeedback',
				'reverbGain',
			]) {
				nodes.current.insertNode(name, ctx.createGain());
			}
			for (const name of ['echoDelay']) {
				nodes.current.insertNode(name, ctx.createDelay());
			}
			for (const name of ['compressor']) {
				nodes.current.insertNode(name, ctx.createDynamicsCompressor());
			}
			for (const name of ['analyser']) {
				nodes.current.insertNode(name, ctx.createAnalyser());
			}
			for (const name of ['lowCutFilter', 'presenceFilter', 'echoFilter']) {
				nodes.current.insertNode(name, ctx.createBiquadFilter());
			}
			for (const name of ['convolver']) {
				nodes.current.insertNode(name, ctx.createConvolver());
			}

			// static setting
			nodes.current.getBiquadFilterNode('lowCutFilter').type = 'highpass';
			nodes.current.getBiquadFilterNode('lowCutFilter').frequency.value = 150;
			nodes.current.getBiquadFilterNode('presenceFilter').type = 'peaking';
			nodes.current.getBiquadFilterNode(
				'presenceFilter'
			).frequency.value = 3500;
			nodes.current.getBiquadFilterNode('presenceFilter').Q.value = 1.2;
			nodes.current.getBiquadFilterNode('presenceFilter').gain.value = 4;
			nodes.current.getBiquadFilterNode('echoFilter').type = 'lowpass';
			nodes.current.getBiquadFilterNode('echoFilter').frequency.value = 3000;
			nodes.current.getConvolverNode('convolver').buffer = generateFakeIRBuffer(
				ctx,
				0.5
			);

			// dynamic setting
			applyVolumeSettings();

			// connection
			nodes.current.connectionList([
				'micSource',
				'lowCutFilter',
				'presenceFilter',
				'micGain',
				'compressor',
			]);
			nodes.current.connectionList([
				'micGain',
				'echoDelay',
				'echoFeedback',
				'echoFilter',
				'echoDelay',
			]);
			nodes.current.connection('echoFeedback', 'compressor');
			nodes.current.connectionList([
				'micGain',
				'convolver',
				'reverbGain',
				'compressor',
			]);
			nodes.current.connection('musicGain', 'compressor');
			nodes.current.connection('compressor', 'audioDestNode');

			setIsEngineRunning(true);
		} catch (err) {
			console.error('Audio failed:', err);
			alert('請確保已開啟麥克風權限');
		}
	};

	const stopAudio = async () => {
		nodes.current.stopAll();
		setIsPlaying(false);

		if (audioCtx.current) {
			await audioCtx.current.close();
			audioCtx.current = null;
		}

		nodes.current = new MyAudioGraph();
		setIsEngineRunning(false);
	};

	const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
		const file = e.target.files?.[0];
		if (!file || !audioCtx.current) return;

		try {
			const arrayBuffer = await file.arrayBuffer();
			const decodedData = await audioCtx.current.decodeAudioData(arrayBuffer);
			setAudioBuffer(decodedData);
			setPausedAt(0); // 重置播放進度
			setIsPlaying(false);
		} catch (error) {
			console.error('音訊解碼失敗:', error);
		}
	};

	const togglePlay = () => {
		if (!audioCtx.current || !audioBuffer) return;

		if (isPlaying) {
			// 暫停：紀錄目前播放位置並停止節點
			const elapsed = audioCtx.current.currentTime - startTime;
			setPausedAt(elapsed);
			nodes.current.getAudioBufferSourceNode('musicSource').stop();
			setIsPlaying(false);
		} else {
			// 播放：建立新節點並從上次位置開始
			const source = audioCtx.current.createBufferSource();
			source.buffer = audioBuffer;
			source.loop = true;
			source.connect(nodes.current.getGainNode('musicGain'));
			nodes.current.insertNode('musicSource', source);

			// 計算 offset (處理循環播放的情況)
			const offset = pausedAt % audioBuffer.duration;
			source.start(0, offset);

			setStartTime(audioCtx.current.currentTime - offset);
			setIsPlaying(true);
		}
	};

	const handleVolumeChange = (key: keyof KTVVolume, value: number) => {
		setVolumeState((prev) => ({ ...prev, [key]: value }));
	};

	return (
		<main className='min-h-screen bg-slate-50 p-4 md:p-8 pb-24'>
			<div className='max-w-md mx-auto space-y-6'>
				{/* Header */}
				<header className='flex justify-between items-end'>
					<div>
						<h1 className='text-2xl font-black text-slate-800 tracking-tight'>
							MY<span className='text-amber-500'>KTV</span>
						</h1>
						<p className='text-[10px] font-bold text-slate-400 uppercase'>
							Powered By Power Bunny
						</p>
					</div>
					<div
						className={`px-2 py-1 rounded-md text-[10px] font-mono border ${
							isEngineRunning
								? 'bg-emerald-50 text-emerald-600 border-emerald-200'
								: 'bg-slate-100 text-slate-400'
						}`}
					>
						{isEngineRunning ? '● ENGINE ACTIVE' : 'OFFLINE'}
					</div>
				</header>

				{/* 1. 麥克風啟動區 */}
				<section>
					<button
						onClick={() => {
							if (isEngineRunning) {
								// 停止
								stopAudio();
							} else {
								// 啟動
								initAudio();
							}
						}}
						className={`w-full h-24 rounded-2xl flex items-center justify-center gap-4 transition-all border-2 ${
							isEngineRunning
								? 'bg-white border-emerald-100 text-emerald-500'
								: 'bg-amber-500 border-amber-600 text-white shadow-lg active:scale-[0.98]'
						}`}
					>
						<Mic size={28} className={isEngineRunning ? 'animate-pulse' : ''} />
						<span className='font-bold'>
							{isEngineRunning ? '麥克風已就緒' : '啟動麥克風引擎'}
						</span>
					</button>
				</section>

				{/* 2. 音樂控制區 (連動 isEngineRunning) */}
				<section
					className={`space-y-4 transition-opacity ${
						!isEngineRunning ? 'opacity-40 pointer-events-none' : 'opacity-100'
					}`}
				>
					<div className='bg-white p-4 rounded-2xl border border-slate-200 shadow-sm'>
						<div className='flex items-center justify-between mb-4'>
							<h3 className='text-sm font-bold flex items-center gap-2'>
								<Music size={16} /> 背景音樂
							</h3>
							<label className='text-xs bg-slate-100 px-3 py-1.5 rounded-full cursor-pointer hover:bg-slate-200 transition-colors'>
								{audioBuffer ? '更換檔案' : '選擇檔案'}
								<input
									type='file'
									className='hidden'
									accept='audio/*'
									onChange={handleFileUpload}
								/>
							</label>
						</div>

						{audioBuffer && (
							<div className='flex items-center gap-3'>
								<button
									onClick={togglePlay}
									className='flex-1 py-3 rounded-xl bg-slate-900 text-white flex items-center justify-center gap-2 active:scale-95 transition-transform'
								>
									{isPlaying ? (
										<>
											<Pause size={18} /> 暫停
										</>
									) : (
										<>
											<Play size={18} /> 播放
										</>
									)}
								</button>
								<button
									onClick={handleReset}
									className={`p-3 rounded-xl border transition-all duration-200 ${
										isResetting
											? 'bg-amber-100 border-amber-400 text-amber-600 scale-90' // 亮起時的樣式
											: 'bg-white border-slate-200 text-slate-400 hover:text-slate-600' // 平時樣式
									}`}
								>
									<RotateCcw
										size={18}
										className={
											isResetting ? 'rotate-[-180deg] transition-transform' : ''
										}
									/>
								</button>
							</div>
						)}
					</div>
				</section>

				{/* 3. 混音器混響區 */}
				<section
					className={`space-y-4 pb-20 transition-opacity ${
						!isEngineRunning ? 'opacity-40 pointer-events-none' : ''
					}`}
				>
					{/* 音量設定 (Gain) */}
					<div className='bg-white p-6 rounded-3xl border border-slate-200 shadow-sm space-y-6'>
						<h3 className='text-[10px] font-black text-slate-400 uppercase tracking-[0.2em] mb-2'>
							Spatial Effects
						</h3>

						<ControlSlider
							label='mic 音量'
							value={volume.mic}
							min={0}
							max={1}
							step={0.01}
							onChange={(v) => handleVolumeChange('mic', v)}
							color='accent-indigo-500'
						/>

						<ControlSlider
							label='music 音量'
							value={volume.music}
							min={0}
							max={0.8}
							step={0.01}
							onChange={(v) => handleVolumeChange('music', v)}
							color='accent-emerald-500'
						/>
					</div>

					{/* 空間效果 (Echo & Reverb) */}
					<div className='bg-white p-6 rounded-3xl border border-slate-200 shadow-sm space-y-6'>
						<h3 className='text-[10px] font-black text-slate-400 uppercase tracking-[0.2em] mb-2'>
							Spatial Effects
						</h3>

						<ControlSlider
							label='Reverb Intensity'
							value={volume.reverb}
							min={0}
							max={1}
							step={0.01}
							onChange={(v) => handleVolumeChange('reverb', v)}
							color='accent-indigo-500'
						/>

						<ControlSlider
							label='Echo Feedback'
							value={volume.echo}
							min={0}
							max={0.8}
							step={0.01}
							onChange={(v) => handleVolumeChange('echo', v)}
							color='accent-emerald-500'
						/>

						<ControlSlider
							label='Echo Delay (s)'
							value={volume.delay}
							min={0.05}
							max={1}
							step={0.01}
							onChange={(v) => handleVolumeChange('delay', v)}
							color='accent-emerald-500'
						/>
					</div>

					{/* 動態處理器 (Compressor) */}
					<div className='bg-slate-900 p-6 rounded-3xl shadow-xl space-y-6 text-white'>
						<h3 className='text-[10px] font-black text-slate-500 uppercase tracking-[0.2em] mb-2'>
							Master Dynamics (Compressor)
						</h3>

						<div className='grid grid-cols-2 gap-4'>
							<ControlSlider
								label='Threshold'
								value={volume.threshold}
								min={-60}
								max={0}
								step={1}
								onChange={(v) => handleVolumeChange('threshold', v)}
								unit='dB'
							/>
							<ControlSlider
								label='Ratio'
								value={volume.ratio}
								min={1}
								max={20}
								step={0.1}
								onChange={(v) => handleVolumeChange('ratio', v)}
								unit=':1'
							/>
							<ControlSlider
								label='Attack'
								value={volume.attack}
								min={0}
								max={0.1}
								step={0.001}
								onChange={(v) => handleVolumeChange('attack', v)}
								unit='s'
							/>
							<ControlSlider
								label='Release'
								value={volume.release}
								min={0.01}
								max={1}
								step={0.01}
								onChange={(v) => handleVolumeChange('release', v)}
								unit='s'
							/>
						</div>
					</div>
				</section>
			</div>
		</main>
	);
}
