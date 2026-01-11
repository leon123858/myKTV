import { KTVVolume } from '../types/types';

export default function ControlSlider({
	label,
	value,
	min,
	max,
	step,
	onChange,
	color = '',
	unit = '',
}: {
	label: string;
	value: number;
	min: number;
	max: number;
	step: number;
	onChange: (value: number) => void;
	color?: string;
	unit?: string;
}) {
	return (
		<div className='space-y-2'>
			<div className='flex justify-between text-[10px] font-bold opacity-70'>
				<span>{label}</span>
				<span className='font-mono'>
					{value}
					{unit}
				</span>
			</div>
			<input
				type='range'
				min={min}
				max={max}
				step={step}
				value={value}
				onChange={(e) => onChange(parseFloat(e.target.value))}
				className={`w-full h-1 bg-slate-200/20 rounded-lg appearance-none cursor-pointer ${color}`}
			/>
		</div>
	);
}
