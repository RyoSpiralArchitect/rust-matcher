import path from "path"
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      // /api/* を sr-api (localhost:3001) に転送
      // CORS 設定不要、開発時のみ有効
      "/api": {
        target: "http://localhost:3001",
        changeOrigin: true,
      },
    },
  },
})
