// app/components/Visualizer.tsx
'use client';

import { useEffect, useRef } from 'react';

interface VisualizerProps {
	analyser: AnalyserNode | null;
	label: string;
	color?: string;
}

export default function Visualizer({
	analyser,
	label,
	color = '#f59e0b',
}: VisualizerProps) {
	const canvasRef = useRef<HTMLCanvasElement>(null);

	useEffect(() => {
		if (!analyser || !canvasRef.current) return;

		const canvas = canvasRef.current;
		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		const bufferLength = analyser.frequencyBinCount;
		const dataArray = new Uint8Array(bufferLength);
		let animationId: number;

		const draw = () => {
			animationId = requestAnimationFrame(draw);

			// 獲取頻譜數據
			analyser.getByteFrequencyData(dataArray);

			// 清除畫布
			ctx.clearRect(0, 0, canvas.width, canvas.height);

			// 繪製背景網格（除錯用）
			ctx.strokeStyle = 'rgba(0,0,0,0.05)';
			ctx.beginPath();
			for (let i = 0; i < canvas.width; i += 20) {
				ctx.moveTo(i, 0);
				ctx.lineTo(i, canvas.height);
			}
			for (let i = 0; i < canvas.height; i += 20) {
				ctx.moveTo(0, i);
				ctx.lineTo(canvas.width, i);
			}
			ctx.stroke();

			// 繪製頻譜
			const barWidth = (canvas.width / bufferLength) * 2.5;
			let x = 0;

			for (let i = 0; i < bufferLength; i++) {
				const barHeight = (dataArray[i] / 255) * canvas.height;
				ctx.fillStyle = color;
				ctx.fillRect(x, canvas.height - barHeight, barWidth, barHeight);
				x += barWidth + 1;
			}
		};

		draw();
		return () => cancelAnimationFrame(animationId);
	}, [analyser, color]);

	return (
		<div className='bg-slate-100 rounded-xl p-2 border border-slate-200'>
			<div className='flex justify-between text-[10px] font-bold text-slate-500 mb-1 px-1'>
				<span>{label}</span>
				<span>FFT: {analyser?.fftSize}</span>
			</div>
			<canvas
				ref={canvasRef}
				width={300}
				height={80}
				className='w-full h-20 rounded bg-white'
			/>
		</div>
	);
}
