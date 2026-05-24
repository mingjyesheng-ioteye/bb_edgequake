import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // ============================================================================
  // Build Performance Optimization
  // Prevents CPU overload during compilation
  // ============================================================================

  // Limit experimental workers to prevent CPU overload
  experimental: {
    // Reduce worker count to prevent memory/CPU exhaustion
    cpus: Math.min(4, typeof process !== "undefined" && process.env.CI ? 2 : 4),
    // Use SWC minifier (faster than Terser)
    webpackBuildWorker: true,
  },

  // TypeScript configuration
  typescript: {
    // Don't fail build on TS errors (we use tsc separately)
    ignoreBuildErrors: false,
  },

  // Turbopack: explicitly set root to the project directory to avoid
  // workspace-root misdetection in monorepo setups.
  turbopack: {
    root: __dirname,
  },

  // Output configuration – static export so the Rust edgequake binary can
  // serve the UI directly via tower-http ServeDir (no Node.js runtime needed).
  output: "export",
  // trailingSlash: generates graph/index.html (not graph.html) so ServeDir
  // can match requests for /graph/ without needing the SPA fallback.
  trailingSlash: true,
  // Static export: disable image optimisation (requires a server)
  images: { unoptimized: true },

  // Reduce logging
  logging: {
    fetches: {
      fullUrl: false,
    },
  },
};

export default nextConfig;
