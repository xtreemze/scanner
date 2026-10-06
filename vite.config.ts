import { defineConfig } from 'vite';
import { VitePWA } from 'vite-plugin-pwa';

export default defineConfig({
  base: './',
  plugins: [
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['scanner-mark.svg'],
      manifest: {
        name: 'Scanner',
        short_name: 'Scanner',
        description: 'Guided real-time spatial reconstruction from mobile sensors.',
        start_url: './',
        scope: './',
        display: 'standalone',
        background_color: '#0b0d10',
        theme_color: '#0b0d10',
        orientation: 'any',
        icons: [
          { src: 'scanner-mark.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' },
          { src: 'scanner-mark.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'maskable' }
        ]
      },
      workbox: {
        navigateFallback: 'index.html',
        globPatterns: ['**/*.{js,css,html,svg,woff2}']
      }
    })
  ],
  build: { target: 'es2024' }
});
