import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_DEBUG ? 'esnext' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      output: {
        // 重量級ライブラリを分離し、初回ロードの並列化とキャッシュ効率を改善する。
        // pdf-lib は annotationService 側で動的 import しているため、ここでは指定せず
        // Rollup の自動分割（遅延ロード可能な独立チャンク）に任せる。
        manualChunks: {
          'vendor-pdfjs': ['pdfjs-dist'],
          'vendor-react': ['react', 'react-dom'],
        },
      },
    },
  },
  optimizeDeps: {
    include: ['pdfjs-dist'],
  },
})
