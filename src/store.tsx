// 全局状态：服务状态、环境体检、设置、后台任务进度（单一数据源）
import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from 'react';
import { api, EnvReport, onProgress, ServiceState, Settings, TaskProgress } from './api';

export interface TaskView { line: string; percent?: number; done: boolean; error?: string }

interface Hub {
  services: ServiceState[];
  env: EnvReport | null;
  settings: Settings | null;
  tasks: Record<string, TaskView>;
  refresh: () => Promise<void>;
  refreshEnv: () => Promise<void>;
  saveSettings: (s: Settings) => Promise<void>;
  clearTask: (id: string) => void;
}
const Ctx = createContext<Hub | null>(null);
export const useHub = () => useContext(Ctx)!;

function parse(p: TaskProgress): TaskView {
  // ollama 进度格式「42%|status」；docker 输出直接展示
  const m = /^(\d+)%\|(.*)$/.exec(p.line);
  return m ? { line: m[2], percent: Number(m[1]), done: p.done, error: p.error } : { line: p.line, done: p.done, error: p.error };
}

function applyTheme(theme: Settings['theme']) {
  const root = document.documentElement;
  if (theme === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
}

export function HubProvider({ children }: { children: ReactNode }) {
  const [services, setServices] = useState<ServiceState[]>([]);
  const [env, setEnv] = useState<EnvReport | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [tasks, setTasks] = useState<Record<string, TaskView>>({});

  const refresh = useCallback(async () => setServices(await api.listServices()), []);
  const refreshEnv = useCallback(async () => setEnv(await api.checkEnv()), []);

  useEffect(() => {
    api.getSettings().then((s) => { setSettings(s); applyTheme(s.theme); });
    refreshEnv();
    refresh();
    const t = window.setInterval(refresh, 5000);
    const off = onProgress((p) => {
      setTasks((prev) => ({ ...prev, [p.id]: parse(p) }));
      if (p.done) refresh();
    });
    return () => { window.clearInterval(t); off.then((f) => f()); };
  }, [refresh, refreshEnv]);

  const saveSettings = useCallback(async (s: Settings) => {
    await api.saveSettings(s);
    setSettings(s);
    applyTheme(s.theme);
  }, []);

  const clearTask = useCallback((id: string) => setTasks((prev) => { const n = { ...prev }; delete n[id]; return n; }), []);

  return <Ctx.Provider value={{ services, env, settings, tasks, refresh, refreshEnv, saveSettings, clearTask }}>{children}</Ctx.Provider>;
}

export const statusTag = (s?: ServiceState['status'], busy?: boolean): { tone: 'ok' | 'warn' | 'bad' | 'idle' | 'brand'; text: string } => {
  if (busy) return { tone: 'brand', text: '部署中' };
  switch (s) {
    case 'running': return { tone: 'ok', text: '运行中' };
    case 'starting': return { tone: 'warn', text: '启动中' };
    case 'stopped': return { tone: 'idle', text: '已停止' };
    case 'error': return { tone: 'bad', text: '异常' };
    default: return { tone: 'idle', text: '未安装' };
  }
};
