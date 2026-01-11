import type { NextConfig } from 'next';

const nextConfig: NextConfig = {
	output: 'export',
	basePath: process.env.NODE_ENV === 'production' ? '/leon123858/myKTV/' : '',
	images: {
		unoptimized: true,
	},
	trailingSlash: true,
};

export default nextConfig;
