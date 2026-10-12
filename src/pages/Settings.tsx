import { useEffect, useState } from 'react';
import { inTauri, Settings as S } from '../api';
import { Button, Group, Segmented, Switch, useUi } from '../components/design';
import { useHub } from '../store';

const MIRRORS = [
  { value: '', label: '不使用（直连 Docker Hub）' },
  { value: 'docker.m.daocloud.io', label: 'DaoCloud 加速' },
  { value: 'docker.1ms.run', label: '毫秒镜像' }
];

export function Settings() {
  const { settings, saveSettings } = useHub();
  const { toast } = useUi();
  const [draft, setDraft] = useState<S | null>(settings);
  useEffect(() => setDraft(settings), [settings]);
  if (!draft || !settings) return null;

  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);
  const set = <K extends keyof S>(k: K, v: S[K]) => setDraft({ ...draft, [k]: v });

  const setTheme = (theme: S['theme']) => saveSettings({ ...settings, theme }); // 主题即时生效
  const setAutostart = async (on: boolean) => {
    if (inTauri) {
      const m = await import('@tauri-apps/plugin-autostart');
      await (on ? m.enable() : m.disable());
    }
    await saveSettings({ ...settings, autostart: on });
  };
  const pickDataDir = async () => {
    if (!inTauri) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const dir = await open({ directory: true, defaultPath: draft.dataDir, title: '选择数据目录' });
    if (typeof dir === 'string') set('dataDir', dir);
  };
  const save = async () => {
    if (!Number.isInteger(draft.gatewayPort) || draft.gatewayPort < 1024 || draft.gatewayPort > 65535) { toast('端口需在 1024–65535 之间'); return; }
    await saveSettings(draft);
    toast('已保存');
  };

  return (
    <div className="page">
      <h1 className="page-title">设置</h1>
      <p className="page-sub">外观即时生效；网络与下载设置需点底部「保存」。</p>

      <Group title="外观与启动">
        <div className="row">
          <div className="row-main"><div className="row-title">主题</div></div>
          <Segmented value={settings.theme} onChange={setTheme} options={[{ value: 'system', label: '跟随系统' }, { value: 'light', label: '浅色' }, { value: 'dark', label: '深色' }]} />
        </div>
        <div className="row">
          <div className="row-main"><div className="row-title">开机自动启动</div><div className="row-desc">让手机随时能连上网关；服务容器本身由 Docker Desktop 自动拉起。</div></div>
          <Switch on={settings.autostart} onChange={setAutostart} label="开机自动启动" />
        </div>
      </Group>

      <Group title="网关">
        <div className="row">
          <div className="row-main"><div className="row-title">网关端口</div><div className="row-desc">知舟访问服务台的端口，修改后需同步更新知舟中的地址。</div></div>
          <input className="input" style={{ width: 120 }} value={draft.gatewayPort} onChange={(e) => set('gatewayPort', Number(e.target.value) || 0)} />
        </div>
      </Group>

      <Group title="下载加速（国内网络推荐）">
        <div className="row">
          <div className="row-main"><div className="row-title">Docker 镜像加速</div><div className="row-desc">用于 Ollama、SearXNG 等 Docker Hub 镜像；已安装的服务需重新部署后生效。</div></div>
          <select className="input" style={{ width: 240 }} value={draft.registryMirror} onChange={(e) => set('registryMirror', e.target.value)}>
            {MIRRORS.map((m) => <option key={m.value} value={m.value}>{m.label}</option>)}
            {!MIRRORS.some((m) => m.value === draft.registryMirror) && <option value={draft.registryMirror}>{draft.registryMirror}</option>}
          </select>
        </div>
        <div className="row">
          <div className="row-main"><div className="row-title">Python 软件源</div><div className="row-desc">构建 MinerU 镜像时使用。</div></div>
          <input className="input" style={{ width: 320 }} value={draft.pipIndex} onChange={(e) => set('pipIndex', e.target.value)} />
        </div>
      </Group>

      <Group title="数据">
        <div className="row">
          <div className="row-main">
            <div className="row-title">数据目录</div>
            <div className="row-desc" style={{ fontFamily: 'var(--mono)' }}>{draft.dataDir}</div>
            <div className="row-meta">存放服务配置（如 SearXNG）。镜像与模型存在 Docker Desktop 的磁盘镜像里，在 Docker Desktop「设置 → Resources → Advanced」中调整。更改后已安装的网络搜索需重新部署。</div>
          </div>
          <Button small onClick={pickDataDir}>更改</Button>
        </div>
      </Group>

      <Button kind="emph" disabled={!dirty} onClick={save}>保存</Button>
    </div>
  );
}
