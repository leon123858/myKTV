export function generateFakeIRBuffer(ctx: AudioContext, roomSizeConst = 0.5) {
	const irLength = ctx.sampleRate * 2; // 2秒混響
	const irBuffer = ctx.createBuffer(2, irLength, ctx.sampleRate);
	for (let ch = 0; ch < 2; ch++) {
		const data = irBuffer.getChannelData(ch);
		for (let i = 0; i < irLength; i++) {
			// 指數衰減白噪音: (Math.random()*2-1) * e^(-i/tau)
			data[i] =
				(Math.random() * 2 - 1) *
				Math.exp(-i / (ctx.sampleRate * roomSizeConst));
		}
	}
	return irBuffer;
}
