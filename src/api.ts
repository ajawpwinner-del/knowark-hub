// 前后端接口：Tauri 中调用 Rust 命令；纯浏览器预览（npm run dev 直接打开）时使用演示数据
import catalogJson from './catalog/services.json';

export interface ServiceDef {
  id: string; name: string; engine: string; category: string; summary: string; detail: string;
  image: string; imageGpu?: string; containerPort: number; hostPort: number; gpu: 'none' | 'optional' | 'required';
  needs: { ramGb: number; diskGb: number; note: string };
}
export interface ModelDef { name: string; dims?: number; size: string; note: string }
export const catalog = catalogJson as unknown as { services: ServiceDef[]; embeddingModels: ModelDef[]; chatModels: ModelDef[] };

export type EnvStatus = 'ok' | 'warn' | 'missing';
export interface EnvItem { id: string; label: string; status: EnvStatus; detail: string; actionLabel?: string; actionUrl?: string }
export interface EnvReport { items: EnvItem[]; dockerReady: boolean; hasNvidia: boolean; ramGb: number }
export type ServiceStatus = 'not_installed' | 'stopped' | 'running' | 'starting' | 'error';
export interface ServiceState { id: string; status: ServiceStatus; hostPort: number; image: string; useGpu: boolean; detail: string }
export interface Health { ok: boolean; detail: string; ms: number }
export interface LibraryModel { name: string; description: string; sizes: string[]; capabilities: string[]; pulls: string }
export interface LocalModel { name: string; sizeBytes: number; family: string }
export interface LanAddress { ip: string; adapter: string; primary: boolean }
export interface ConnectInfo { addresses: LanAddress[]; gatewayPort: number; gatewayError?: string; key: string; firewallReady: boolean }
export interface ServiceOverride { hostPort?: number; image?: string; useGpu: boolean; options?: Record<string, string> }
export interface SearchHit { title: string; url: string; content: string; engine: string }
export interface ParseResult { markdown: string; pageCount?: number; ms: number; truncated: boolean }
export interface Settings {
  theme: 'system' | 'light' | 'dark'; gatewayPort: number; dataDir: string; registryMirror: string; pipIndex: string;
  autostart: boolean; services: Record<string, ServiceOverride>;
}
export interface TaskProgress { id: string; stage: string; line: string; done: boolean; error?: string }

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  return demo(cmd, args) as T;
}

export const api = {
  checkEnv: () => call<EnvReport>('check_env'),
  listServices: () => call<ServiceState[]>('list_services'),
  install: (id: string) => call<void>('install_service', { id }),
  action: (id: string, action: 'start' | 'stop' | 'restart') => call<void>('service_action', { id, action }),
  uninstall: (id: string, removeData: boolean) => call<void>('uninstall_service', { id, removeData }),
  logs: (id: string) => call<string>('service_logs', { id }),
  probe: (id: string) => call<Health>('probe_service', { id }),
  listModels: () => call<LocalModel[]>('list_models'),
  pullModel: (name: string) => call<void>('pull_model', { name }),
  deleteModel: (name: string) => call<void>('delete_model', { name }),
  modelDims: (name: string) => call<number>('model_dims', { name }),
  modelLibrary: (kind: 'embedding' | 'chat') => call<LibraryModel[]>('model_library', { kind }),
  applyOptions: (id: string) => call<void>('apply_service_options', { id }),
  trySearch: (q: string) => call<SearchHit[]>('try_search', { q }),
  tryParse: (path: string, engine: string) => call<ParseResult>('try_parse', { path, engine }),
  connectInfo: () => call<ConnectInfo>('connect_info'),
  rotateKey: () => call<string>('rotate_key'),
  addFirewallRule: () => call<void>('add_firewall_rule'),
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<void>('save_settings', { settings })
};

export async function onProgress(cb: (p: TaskProgress) => void): Promise<() => void> {
  if (inTauri) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<TaskProgress>('task-progress', (e) => cb(e.payload));
  }
  demoListeners.add(cb);
  return () => demoListeners.delete(cb);
}

export async function pickFile(kind: 'image' | 'document'): Promise<string | null> {
  if (!inTauri) return 'C:\demo\sample.' + (kind === 'image' ? 'png' : 'pdf');
  const { open } = await import('@tauri-apps/plugin-dialog');
  const filters = kind === 'image'
    ? [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'bmp', 'webp'] }]
    : [{ name: '文档', extensions: ['pdf', 'docx', 'pptx'] }];
  const f = await open({ multiple: false, filters });
  return typeof f === 'string' ? f : null;
}

export async function openUrl(url: string) {
  if (inTauri) {
    const { openUrl: open } = await import('@tauri-apps/plugin-opener');
    await open(url);
  } else window.open(url, '_blank');
}

export async function saveTextFile(name: string, text: string) {
  if (inTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: name, filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!path) return false;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('write_text_file', { path, text });
    return true;
  }
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
  a.download = name;
  a.click();
  return true;
}

// ---------- 浏览器预览用演示数据 ----------
const demoListeners = new Set<(p: TaskProgress) => void>();
const demoState: Record<string, ServiceStatus> = { ollama: 'running', searxng: 'running', mineru: 'not_installed' };
let demoSettings: Settings = {
  theme: 'system', gatewayPort: 8765, dataDir: 'C:\\Users\\me\\AppData\\Local\\KnowArkHub', registryMirror: '',
  pipIndex: 'https://pypi.tuna.tsinghua.edu.cn/simple', autostart: false, services: {}
};

function demoTask(id: string, then: () => void) {
  let n = 0;
  const t = setInterval(() => {
    n += 12;
    const done = n >= 100;
    demoListeners.forEach((cb) => cb({ id, stage: 'pull', line: done ? '完成' : `${Math.min(n, 99)}%|downloading`, done }));
    if (done) { clearInterval(t); then(); }
  }, 300);
}

function demo(cmd: string, args?: Record<string, unknown>): unknown {
  const id = args?.id as string;
  switch (cmd) {
    case 'check_env':
      return {
        dockerReady: true, hasNvidia: false, ramGb: 16,
        items: [
          { id: 'os', label: '操作系统', status: 'ok', detail: 'Windows 11 家庭中文版' },
          { id: 'ram', label: '内存', status: 'ok', detail: '16 GB，可以同时运行全部服务' },
          { id: 'disk', label: '磁盘空间', status: 'ok', detail: 'C: 盘剩余 182 GB' },
          { id: 'wsl', label: 'WSL 2', status: 'ok', detail: '已启用' },
          { id: 'docker', label: 'Docker Desktop', status: 'ok', detail: '版本 27.3.1' },
          { id: 'gpu', label: 'NVIDIA 显卡', status: 'warn', detail: '未检测到：服务将以 CPU 运行，速度较慢但可用' }
        ]
      } satisfies EnvReport;
    case 'list_services':
      return catalog.services.map((s) => ({ id: s.id, status: demoState[s.id], hostPort: s.hostPort, image: s.image, useGpu: false, detail: '' }));
    case 'install_service':
      demoTask(id, () => { demoState[id] = 'running'; });
      return undefined;
    case 'service_action':
      demoState[id] = args?.action === 'stop' ? 'stopped' : 'running';
      return undefined;
    case 'uninstall_service':
      demoState[id] = 'not_installed';
      return undefined;
    case 'service_logs':
      return '2026-10-12T09:00:01Z 服务启动\n2026-10-12T09:00:02Z listening on 0.0.0.0';
    case 'probe_service':
      return { ok: true, detail: '运行正常', ms: 182 };
    case 'list_models':
      return [{ name: 'bge-m3:latest', sizeBytes: 1_200_000_000, family: 'bert' }];
    case 'pull_model':
      demoTask(`model:${args?.name}`, () => undefined);
      return undefined;
    case 'try_search':
      return [{ title: `${args?.q} - 维基百科`, url: 'https://zh.wikipedia.org/', content: '这是演示数据：真实环境会显示 SearXNG 返回的搜索结果摘要。', engine: 'wikipedia' }];
    case 'try_parse':
      return { markdown: '# 演示文档\n\n真实环境会显示 MinerU 的识别结果。\n\n| 列 | 值 |\n|---|---|\n| 示例 | 1 |', pageCount: 1, ms: 1830, truncated: false };
    case 'apply_service_options':
      return undefined;
    case 'model_library':
      return (args?.kind === 'embedding' ? catalog.embeddingModels : catalog.chatModels)
        .map((m) => ({ name: m.name.split(':')[0], description: m.note, sizes: m.name.includes(':') ? [m.name.split(':')[1]] : [], capabilities: [], pulls: '' }));
    case 'model_dims':
      return 1024;
    case 'connect_info':
      return { addresses: [{ ip: '192.168.1.23', adapter: 'WLAN', primary: true }], gatewayPort: demoSettings.gatewayPort, key: 'ka-demo0000000000000000000000000000000000', firewallReady: false };
    case 'rotate_key':
      return 'ka-demo' + Math.random().toString(36).slice(2).padEnd(34, '0');
    case 'get_settings':
      return demoSettings;
    case 'save_settings':
      demoSettings = args?.settings as Settings;
      return undefined;
    default:
      return undefined;
  }
}
