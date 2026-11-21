import { readFileSync } from 'node:fs'
import { networkInterfaces } from 'node:os'
import { resolve } from 'node:path'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig, loadEnv, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'

/** 本机内网 IPv4：取第一个非回环地址，与后端启动摘要保持一致 */
function localIp(): string {
  const nets = networkInterfaces()
  for (const list of Object.values(nets)) {
    for (const net of list ?? []) {
      if (net.family === 'IPv4' && !net.internal) return net.address
    }
  }
  return 'localhost'
}

/** 从后端 .env 读管理员账号，读不到就返回空 */
function adminFromEnv(): { user: string; password: string } {
  const candidates = [
    resolve(process.cwd(), '../backend/.env'),
    resolve(process.cwd(), '.env'),
  ]
  for (const file of candidates) {
    try {
      const text = readFileSync(file, 'utf8')
      const pick = (key: string): string => {
        const match = text.match(new RegExp(`^${key}\\s*=\\s*(.+)$`, 'm'))
        return match ? match[1].trim() : ''
      }
      const user = pick('ATLAS_ADMIN_USER') || 'admin'
      const password = pick('ATLAS_ADMIN_PASSWORD')
      if (password) return { user, password }
    } catch {
      // 换下一个候选
    }
  }
  return { user: 'admin', password: '' }
}

/** 启动时打印摘要：版本、可访问的前端地址、管理员 */
function startupBanner(devPort: string, version: string): Plugin {
  return {
    name: 'atlas-startup-banner',
    apply: 'serve',
    configureServer(server) {
      server.httpServer?.once('listening', () => {
        const port = devPort
        const admin = adminFromEnv()
        const lines = [
          '',
          `RedTeam Atlas ${version}`,
          `http://${localIp()}:${port}`,
          '',
          '管理员',
          `  用户名: ${admin.user}`,
          `  口令  : ${admin.password || '见后端启动日志，或在「用户管理」里重置'}`,
          '',
        ]
        // 等 Vite 打完 ready 再输出，摘要落在最后更像一张启动卡片
        setTimeout(() => {
          for (const line of lines) server.config.logger.info(line, { timestamp: false })
        }, 30)
      })
    },
  }
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const apiTarget = env.VITE_API_TARGET || 'http://127.0.0.1:8080'
  const pkg = JSON.parse(readFileSync(resolve(process.cwd(), 'package.json'), 'utf8')) as { version: string }

  return {
    plugins: [vue(), startupBanner(String(Number(env.VITE_DEV_PORT || 5173)), pkg.version)],
    // 相对路径打包，便于放在任意子目录或反向代理下
    base: './',
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url)),
      },
    },
    server: {
      // 监听 0.0.0.0：只绑 localhost 时同网段的其他机器打不开
      host: true,
      port: Number(env.VITE_DEV_PORT || 5173),
      proxy: {
        '/api': { target: apiTarget, changeOrigin: true },
        '/mcp': { target: apiTarget, changeOrigin: true },
      },
      // 避免监听构建产物与本地数据目录
      watch: { ignored: ['**/dist/**', '**/node_modules/**', '**/data/**'] },
    },
    build: {
      outDir: 'dist',
      sourcemap: false,
      chunkSizeWarningLimit: 900,
      rollupOptions: {
        output: {
          manualChunks: {
            vendor: ['vue', 'vue-router', 'pinia', 'axios'],
            flow: ['@vue-flow/core', '@vue-flow/background', '@vue-flow/controls', '@vue-flow/minimap'],
          },
        },
      },
    },
  }
})