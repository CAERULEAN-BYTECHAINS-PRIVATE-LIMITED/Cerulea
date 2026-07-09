/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,

  typescript: { ignoreBuildErrors: true },
  eslint: { ignoreDuringBuilds: true },

  // Native modules must not be bundled by webpack
  // (serverExternalPackages is Next.js 15+; use experimental key for 14.x)
  experimental: {
    serverComponentsExternalPackages: ['bcrypt'],
  },

  output: 'standalone',
  compress: true,
  productionBrowserSourceMaps: false,

  // Only proxy /api if you explicitly enable it.
  async rewrites() {
    if (process.env.NEXT_PUBLIC_API_PROXY === '1') {
      return [
        {
          source: '/api/:path*',
          destination: 'http://localhost:4000/api/:path*',
        },
      ];
    }
    return [];
  },

  images: {
    remotePatterns: [
      { protocol: 'https', hostname: 'cerulea-backend.onrender.com' },
    ],
  },

  async headers() {
    return [
      {
        source: '/:path*',
        headers: [
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
          { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=()' },
        ],
      },
    ];
  },
};

module.exports = nextConfig;
