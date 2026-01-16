export const IRFilePath =
	process.env.NODE_ENV === 'production' ? '/myKTV/plate01.wav' : '/plate01.wav';

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

/**
 * get IR buffer from file
 * @param ctx 當前的 AudioContext
 * @param audioFile 使用者上傳的 File 物件
 */
export async function generateIRFromAudioFile(
	ctx: AudioContext,
	audioFile: File
): Promise<AudioBuffer> {
	const arrayBuffer = await audioFile.arrayBuffer();
	const decodedBuffer = await ctx.decodeAudioData(arrayBuffer);

	return decodedBuffer;
}

/**
 * 將公用路徑的靜態資源轉化為 File 物件
 * @param path 檔案路徑，例如 '/plate01.wav'
 * @param fileName 想要賦予 File 物件的名稱
 */
export async function getFileFromStaticPath(
	path: string,
	fileName = 'ir.wav'
): Promise<File> {
	// 1. Fetch 該資源
	const response = await fetch(path);

	if (!response.ok) {
		throw new Error(`無法讀取資源: ${path}`);
	}

	// 2. 轉為 Blob
	const blob = await response.blob();

	// 3. 封裝成 File 物件
	// 第三個參數可以設定最後修改時間與 MIME Type
	return new File([blob], fileName, {
		type: 'audio/wav',
		lastModified: Date.now(),
	});
}
