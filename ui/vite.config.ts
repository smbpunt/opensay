import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Prevent Vite from obscuring Rust errors
  clearScreen: false,
  // Tauri expects a fixed port
  server: {
    port: 1420,
    strictPort: true,
  },
  // Env variables prefixed with TAURI_ are exposed to the frontend
  envPrefix: ['VITE_', 'TAURI_'],
})
