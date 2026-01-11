'use client';

import { useState, useRef } from 'react';
import { Mic, Music } from 'lucide-react';
import { KTVNode, KTVVolume } from '../types/types';

export default function KTVPage() {
	const [isEngineRunning, setIsEngineRunning] = useState(false);
	const [volume] = useState<KTVVolume>({
		mic: 0.8,
		music: 0.6,
		echo: 0.3,
		delay: 0.2,
		ratio: 12,
		ducking: -35,
	});

	// 用 useRef 保持 Audio Node 引用，避免 Re-render 導致中斷
	const audioCtx = useRef<AudioContext | null>(null);
	const node = useRef<KTVNode>({});
	const musicSource = useRef<AudioBufferSourceNode | null>(null);

	const setVolume = (rampTime = 0.05) => {
		const ctx = audioCtx.current;
		const n = node.current;

		if (!ctx) return;

		const { mic, music, echo, delay, ratio, ducking } = volume;
		const now = ctx.currentTime;

		// 1. 增益類參數 (GainNodes)
		n.micGain?.gain.setTargetAtTime(mic, now, rampTime);
		n.musicGain?.gain.setTargetAtTime(music, now, rampTime);
		n.echoFeedback?.gain.setTargetAtTime(echo, now, rampTime);

		// 2. 時間類參數 (DelayNode)
		// 注意：delayTime 若劇烈變動會產生類比磁帶轉速改變的音效 (Pitch shift)
		n.echoDelay?.delayTime.setTargetAtTime(delay, now, rampTime);

		// 3. 動態處理類參數 (DynamicsCompressorNode)
		if (n.compressor) {
			// 壓縮比 (Ratio)
			n.compressor.ratio.setTargetAtTime(ratio, now, rampTime);
			// 閃避門檻 (Threshold) - 控制音樂被壓低的靈敏度
			n.compressor.threshold.setTargetAtTime(ducking, now, rampTime);
		}
	};

	// 初始化 Web Audio 節點圖 (Audio Graph)
	const initAudio = async () => {
		if (audioCtx.current) return;

		try {
			const Context = window.AudioContext;
			audioCtx.current = new Context();
			const ctx = audioCtx.current;

			const stream = await navigator.mediaDevices.getUserMedia({
				audio: {
					// note: 48000 44100, 22050 is quality options
					sampleRate: { ideal: 48000 },
					// note: cancel echo
					echoCancellation: true,
					// turn off mechanism to lock volume auto
					autoGainControl: false,
					// try to rm noise
					noiseSuppression: { ideal: true },
					channelCount: 1,
				},
			});

			// 建立節點
			node.current.micSource = ctx.createMediaStreamSource(stream);
			node.current.micGain = ctx.createGain();
			node.current.musicGain = ctx.createGain();
			node.current.echoDelay = ctx.createDelay();
			node.current.echoFeedback = ctx.createGain();
			node.current.compressor = ctx.createDynamicsCompressor();
			node.current.analyser = ctx.createAnalyser();
			node.current.analyser.fftSize = 256;

			// 配置 Ducking (當人聲進來時自動壓低背景音樂)
			node.current.compressor.threshold.setValueAtTime(-35, ctx.currentTime);
			node.current.compressor.ratio.setValueAtTime(12, ctx.currentTime);

			// 連接節點
			// 人聲路徑: Mic -> Gain -> Analyser & Compressor
			node.current.micSource.connect(node.current.micGain);
			node.current.micGain.connect(node.current.analyser);
			node.current.micGain.connect(node.current.compressor);

			// 迴響路徑: MicGain -> Delay -> Feedback -> Delay (迴圈) -> Compressor
			node.current.echoDelay.delayTime.value = 0.2;
			node.current.micGain.connect(node.current.echoDelay);
			node.current.echoDelay.connect(node.current.echoFeedback);
			node.current.echoFeedback.connect(node.current.echoDelay);
			node.current.echoFeedback.connect(node.current.compressor);

			// 最終輸出
			node.current.compressor.connect(ctx.destination);

			setIsEngineRunning(true);
			setVolume();
		} catch (err) {
			console.error('Audio failed:', err);
			alert('請確保已開啟麥克風權限');
		}
	};

	// 定義事件型別
	type UploadEvent = React.ChangeEvent<HTMLInputElement>;

	const handleFileUpload = async (e: UploadEvent) => {
		// 使用 Optional Chaining 與 Guard Clause
		const file = e.target.files?.[0];
		if (!file) return;

		if (!audioCtx.current) {
			await initAudio();
		}

		// confirm audio context is initialized
		const ctx = audioCtx.current;
		if (!ctx) return;

		try {
			const arrayBuffer = await file.arrayBuffer();
			const audioBuffer = await ctx.decodeAudioData(arrayBuffer);

			musicSource.current?.stop();

			const source = ctx.createBufferSource();
			source.buffer = audioBuffer;
			source.loop = true;

			// 取得現有混音節點引用
			const { musicGain, compressor } = node.current;

			if (musicGain && compressor) {
				// 串接：Source -> MusicGain -> Compressor
				source.connect(musicGain);
				source.start();

				musicSource.current = source;
			}
		} catch (error) {
			console.error('音訊解碼失敗:', error);
		}
	};

	return (
		<main className='min-h-screen bg-slate-50 p-4 md:p-8'>
			<div className='max-w-md mx-auto space-y-6'>
				{/* Header */}
				<div className='flex justify-between items-end'>
					<div>
						<h1 className='text-2xl font-black text-slate-800 tracking-tight'>
							KARAOKE<span className='text-amber-500'>NEXT</span>
						</h1>
						<p className='text-[10px] font-bold text-slate-400'>
							MOBILE WEB STUDIO
						</p>
					</div>
					<div
						className={`px-2 py-1 rounded-md text-[10px] font-mono border ${
							isEngineRunning
								? 'bg-emerald-50 text-emerald-600 border-emerald-200'
								: 'bg-slate-100 text-slate-400'
						}`}
					>
						STATUS: {isEngineRunning ? 'RUNNING' : 'OFFLINE'}
					</div>
				</div>

				{/* 核心操作區 */}
				<div className='grid grid-cols-2 gap-4'>
					<button
						onClick={initAudio}
						disabled={isEngineRunning}
						className={`h-32 rounded-3xl flex flex-col items-center justify-center gap-2 transition-all shadow-sm border ${
							isEngineRunning
								? 'bg-white text-slate-300'
								: 'bg-amber-500 text-white active:scale-95 shadow-amber-200'
						}`}
					>
						<Mic size={32} />
						<span className='font-bold text-sm'>啟動麥克風</span>
					</button>

					<label className='h-32 rounded-3xl bg-white border-2 border-dashed border-slate-200 flex flex-col items-center justify-center gap-2 text-slate-500 cursor-pointer active:bg-slate-50'>
						<Music size={32} />
						<span className='font-bold text-sm'>選擇音樂</span>
						<input
							type='file'
							className='hidden'
							accept='audio/*'
							onChange={handleFileUpload}
						/>
					</label>
				</div>
			</div>
		</main>
	);
}
