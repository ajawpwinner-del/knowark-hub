import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri 固定端口；浏览器预览时同样可用（无 Tauri 时走 api.ts 的演示数据）
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // 不监视 Rust 工程：编译中的 dll 被锁会让监视器 EBUSY 崩溃
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: { target: 'es2021', outDir: 'dist' }
});
