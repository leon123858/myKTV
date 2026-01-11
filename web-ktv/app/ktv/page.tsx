'use client';

import { useState, useRef, useEffect } from 'react';
import { Mic, Music, Play, Pause, RotateCcw, Sliders } from 'lucide-react';
import { KTVNode, KTVVolume } from '../types/types';

export default function KTVPage() {
	const [isEngineRunning, setIsEngineRunning] = useState(false);
	const [isPlaying, setIsPlaying] = useState(false);
	const [volume, setVolumeState] = useState<KTVVolume>({
		mic: 0.8,
		music: 0.6,
		echo: 0.3,
		delay: 0.2,
		ratio: 12,
		ducking: -35,
	});

	// 音樂播放相關狀態
	const [audioBuffer, setAudioBuffer] = useState<AudioBuffer | null>(null);
	const [startTime, setStartTime] = useState(0); // 紀錄開始播放的絕對時間
	const [pausedAt, setPausedAt] = useState(0); // 紀錄暫停時已播放了幾秒
	const [isResetting, setIsResetting] = useState(false);

	const audioCtx = useRef<AudioContext | null>(null);
	const node = useRef<KTVNode>({});
	const musicSource = useRef<AudioBufferSourceNode | null>(null);

	useEffect(() => {
		const applyVolumeSettings = (rampTime = 0.05) => {
			const ctx = audioCtx.current;
			const n = node.current;
			if (!ctx) return;

			const { mic, music, echo, delay, ratio, ducking } = volume;
			const now = ctx.currentTime;

			n.micGain?.gain.setTargetAtTime(mic, now, rampTime);
			n.musicGain?.gain.setTargetAtTime(music, now, rampTime);
			n.echoFeedback?.gain.setTargetAtTime(echo, now, rampTime);
			n.echoDelay?.delayTime.setTargetAtTime(delay, now, rampTime);

			if (n.compressor) {
				n.compressor.ratio.setTargetAtTime(ratio, now, rampTime);
				n.compressor.threshold.setTargetAtTime(ducking, now, rampTime);
			}
		};

		if (isEngineRunning) {
			applyVolumeSettings();
		}
	}, [volume, isEngineRunning]);

	const handleReset = () => {
		// 1. 執行原有的邏輯
		musicSource.current?.stop();
		setPausedAt(0);
		setIsPlaying(false);

		// 2. 觸發亮燈效果
		setIsResetting(true);

		// 3. 短暫延遲後關閉亮燈（給使用者視覺反饋的時間）
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

			// 建立節點
			node.current.micSource = ctx.createMediaStreamSource(stream);
			node.current.micGain = ctx.createGain();
			node.current.musicGain = ctx.createGain();
			node.current.echoDelay = ctx.createDelay();
			node.current.echoFeedback = ctx.createGain();
			node.current.compressor = ctx.createDynamicsCompressor();
			node.current.analyser = ctx.createAnalyser();

			// 連接人聲路徑
			node.current.micSource.connect(node.current.micGain);
			node.current.micGain.connect(node.current.compressor);

			// 連接迴響路徑
			node.current.micGain.connect(node.current.echoDelay);
			node.current.echoDelay.connect(node.current.echoFeedback);
			node.current.echoFeedback.connect(node.current.echoDelay);
			node.current.echoFeedback.connect(node.current.compressor);

			// 連接音樂路徑 (音樂也進入 compressor 才能實現 Ducking)
			node.current.musicGain.connect(node.current.compressor);

			// 最終輸出
			node.current.compressor.connect(ctx.destination);

			node.current.stream = stream;
			setIsEngineRunning(true);
		} catch (err) {
			console.error('Audio failed:', err);
			alert('請確保已開啟麥克風權限');
		}
	};

	const stopAudio = async () => {
		// 1. 停止所有麥克風軌道
		if (node.current.stream) {
			node.current.stream.getTracks().forEach((track) => track.stop());
			node.current.stream = undefined;
		}

		// 2. 停止音樂播放
		if (isPlaying) {
			musicSource.current?.stop();
			setIsPlaying(false);
		}

		// 3. 關閉 AudioContext
		if (audioCtx.current) {
			await audioCtx.current.close();
			audioCtx.current = null;
		}

		// 4. 清除節點引用
		node.current = {};

		// 5. 更新狀態
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
			musicSource.current?.stop();
			setIsPlaying(false);
		} else {
			// 播放：建立新節點並從上次位置開始
			const source = audioCtx.current.createBufferSource();
			source.buffer = audioBuffer;
			source.loop = true;
			source.connect(node.current.musicGain!);

			// 計算 offset (處理循環播放的情況)
			const offset = pausedAt % audioBuffer.duration;
			source.start(0, offset);

			setStartTime(audioCtx.current.currentTime - offset);
			musicSource.current = source;
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
					className={`space-y-4 transition-opacity ${
						!isEngineRunning ? 'opacity-40 pointer-events-none' : 'opacity-100'
					}`}
				>
					<div className='bg-white p-6 rounded-3xl border border-slate-200 shadow-sm space-y-6'>
						<div className='flex items-center gap-2 border-b pb-3 border-slate-50'>
							<Sliders size={18} className='text-slate-400' />
							<h3 className='text-sm font-black text-slate-700 uppercase tracking-widest'>
								Mixer Console
							</h3>
						</div>

						{/* 麥克風音量 */}
						<div className='space-y-3'>
							<div className='flex justify-between text-[11px] font-bold text-slate-500'>
								<span>MIC VOLUME</span>
								<span className='font-mono'>
									{(volume.mic * 100).toFixed(0)}%
								</span>
							</div>
							<input
								type='range'
								min='0'
								max='1.5'
								step='0.01'
								value={volume.mic}
								onChange={(e) =>
									handleVolumeChange('mic', parseFloat(e.target.value))
								}
								className='w-full h-1.5 bg-slate-100 rounded-lg appearance-none accent-amber-500 cursor-pointer'
							/>
						</div>

						{/* 音樂音量 */}
						<div className='space-y-3'>
							<div className='flex justify-between text-[11px] font-bold text-slate-500'>
								<span>MUSIC VOLUME</span>
								<span className='font-mono'>
									{(volume.music * 100).toFixed(0)}%
								</span>
							</div>
							<input
								type='range'
								min='0'
								max='1'
								step='0.01'
								value={volume.music}
								onChange={(e) =>
									handleVolumeChange('music', parseFloat(e.target.value))
								}
								className='w-full h-1.5 bg-slate-100 rounded-lg appearance-none accent-blue-500 cursor-pointer'
							/>
						</div>

						{/* 迴響強度 */}
						<div className='space-y-3'>
							<div className='flex justify-between text-[11px] font-bold text-slate-500'>
								<span>ECHO FEEDBACK</span>
								<span className='font-mono'>
									{(volume.echo * 100).toFixed(0)}%
								</span>
							</div>
							<input
								type='range'
								min='0'
								max='0.6'
								step='0.01'
								value={volume.echo}
								onChange={(e) =>
									handleVolumeChange('echo', parseFloat(e.target.value))
								}
								className='w-full h-1.5 bg-slate-100 rounded-lg appearance-none accent-emerald-500 cursor-pointer'
							/>
						</div>
					</div>
				</section>
			</div>
		</main>
	);
}
