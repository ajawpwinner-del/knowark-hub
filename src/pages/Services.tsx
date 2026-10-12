import { useEffect, useState } from 'react';
import { api, catalog, EmbeddingModelInfo, Health, LibraryModel, LocalModel, ServiceDef, ServiceState } from '../api';
import { Button, Group, Segmented, Tag, serviceIcon, useUi } from '../components/design';
import { statusTag, useHub } from '../store';
import { TaskBar } from './Catalog';
import { GuideTab, SettingsTab, TryTab } from './ServiceTabs';
import type { Page } from '../App';

export function Services({ go }: { go: (p: Page) => void }) {
  const { services } = useHub();
  const installed = catalog.services.filter((d) => services.find((s) => s.id === d.id && s.status !== 'not_installed'));
  return (
    <div className="page">
      <h1 className="page-title">我的服务</h1>
      <p className="page-sub">启停、体检、看日志；服务会随 Docker Desktop 自动启动。</p>
      {installed.length === 0 && (
        <div className="card empty">
          <p>还没有安装任何服务</p>
          <Button kind="emph" onClick={() => go('catalog')}>去服务目录</Button>
        </div>
      )}
      {installed.map((d) => <ServicePanel key={d.id} def={d} state={services.find((s) => s.id === d.id)!} go={go} />)}
    </div>
  );
}

type PanelTab = 'overview' | 'settings' | 'try' | 'guide';

function ServicePanel({ def, state, go }: { def: ServiceDef; state: ServiceState; go: (p: Page) => void }) {
  const [tab, setTab] = useState<PanelTab>('overview');
  const { refresh, settings, saveSettings, tasks } = useHub();
  const { confirm, toast } = useUi();
  const [busy, setBusy] = useState<string | null>(null);
  const [health, setHealth] = useState<Health | null>(null);
  const [logs, setLogs] = useState<string | null>(null);
  const [edit, setEdit] = useState(false);
  const [port, setPort] = useState(String(state.hostPort));
  const [image, setImage] = useState(settings?.services[def.id]?.image ?? '');
  const tag = statusTag(state.status, !!tasks[def.id] && !tasks[def.id].done);
  const running = state.status === 'running';

  const run = async (key: string, fn: () => Promise<unknown>) => {
    setBusy(key);
    try { await fn(); await refresh(); } catch (e) { toast(String(e)); } finally { setBusy(null); }
  };

  const probe = () => run('probe', async () => setHealth(await api.probe(def.id)));
  const showLogs = () => run('logs', async () => setLogs(logs === null ? await api.logs(def.id) : null));

  const uninstall = async () => {
    const r = await confirm({
      level: 'L1', title: `卸载${def.name}？`,
      message: '将删除该服务的容器，知舟中对应的功能会无法使用。镜像保留以便日后重装。',
      confirmText: '卸载', checkbox: '同时删除已下载的模型与数据（不可恢复）'
    });
    if (r.ok) run('uninstall', () => api.uninstall(def.id, r.checked));
  };

  const saveConfig = async () => {
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1024 || p > 65535) { toast('端口需在 1024–65535 之间'); return; }
    const r = await confirm({
      level: 'L2', title: '按新配置重新部署？',
      message: '会删除现有容器并用新端口与镜像重新创建，已下载的模型与数据保留。',
      confirmText: '重新部署'
    });
    if (!r.ok || !settings) return;
    const services = { ...settings.services, [def.id]: { ...settings.services[def.id], useGpu: settings.services[def.id]?.useGpu ?? false, hostPort: p, image: image.trim() || undefined } };
    await saveSettings({ ...settings, services });
    setEdit(false);
    await api.install(def.id);
  };

  return (
    <Group
      title={<span style={{ display: 'flex', alignItems: 'center', gap: 8 }}><span style={{ width: 18, height: 18, display: 'inline-flex', color: 'var(--brand-primary)' }}>{serviceIcon(def.category)}</span>{def.name} · {def.engine}</span>}
      extra={<Tag tone={tag.tone}>{tag.text}</Tag>}
    >
      <div className="row">
        <div className="row-main">
          <div className="row-title">本机端口 {state.hostPort}{state.useGpu ? ' · 显卡加速' : ''}</div>
          <div className="row-meta" style={{ fontFamily: 'var(--mono)' }}>{state.image}</div>
          <TaskBar task={tasks[def.id]} />
        </div>
        <div className="row-actions">
          {running
            ? <><Button small busy={busy === 'restart'} onClick={() => run('restart', () => api.action(def.id, 'restart'))}>重启</Button>
                <Button small busy={busy === 'stop'} onClick={() => run('stop', () => api.action(def.id, 'stop'))}>停止</Button></>
            : <Button small kind="emph" busy={busy === 'start'} onClick={() => run('start', () => api.action(def.id, 'start'))}>启动</Button>}
        </div>
      </div>

      <div className="row" style={{ minHeight: 0 }}>
        <Segmented value={tab} onChange={setTab} options={[
          { value: 'overview', label: '概览' }, { value: 'settings', label: '设置' }, { value: 'try', label: '试一试' }, { value: 'guide', label: '使用说明' }
        ]} />
      </div>

      {tab === 'settings' && <SettingsTab def={def} />}
      {tab === 'try' && <TryTab def={def} running={running} />}
      {tab === 'guide' && <GuideTab def={def} go={() => go('connect')} />}

      {tab === 'overview' && <>
      <div className="row">
        <div className="row-main">
          <div className="row-title">知舟可用性检查</div>
          <div className="row-desc">
            {health
              ? <><Tag tone={health.ok ? 'ok' : 'bad'}>{health.ok ? '可用' : '不可用'}</Tag>　{health.detail}<span className="row-meta">　{health.ms} ms</span></>
              : '按知舟实际调用的方式真实请求一次'}
          </div>
        </div>
        <div className="row-actions"><Button small busy={busy === 'probe'} disabled={!running} onClick={probe}>{busy === 'probe' ? '检查中' : '检查'}</Button></div>
      </div>

      {def.id === 'ollama' && running && <OllamaModels />}

      <div className="row" style={{ flexDirection: 'column', alignItems: 'stretch' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div className="row-main"><div className="row-title">更多</div></div>
          <Button small kind="text" busy={busy === 'logs'} onClick={showLogs}>{logs === null ? '查看日志' : '收起日志'}</Button>
          <Button small kind="text" onClick={() => setEdit(!edit)}>{edit ? '收起设置' : '端口与镜像'}</Button>
          <Button small kind="warn" busy={busy === 'uninstall'} onClick={uninstall}>卸载</Button>
        </div>
        {logs !== null && <pre className="logs">{logs || '（暂无日志）'}</pre>}
        {edit && (
          <div style={{ display: 'grid', gridTemplateColumns: '160px 1fr auto', gap: 12, alignItems: 'end', marginTop: 8 }}>
            <div className="field"><label>本机端口</label><input className="input" value={port} onChange={(e) => setPort(e.target.value)} /></div>
            <div className="field"><label>自定义镜像（留空用默认）</label><input className="input" value={image} placeholder={def.image} onChange={(e) => setImage(e.target.value)} /></div>
            <Button kind="emph" onClick={saveConfig}>重新部署</Button>
          </div>
        )}
      </div>
      </>}
    </Group>
  );
}

const fmtPulls = (p: string) => {
  const n = Number(p.replace(/,/g, ''));
  if (!n) return p;
  return n >= 1e6 ? `${(n / 1e6).toFixed(1)}M 次下载` : n >= 1e3 ? `${(n / 1e3).toFixed(0)}K 次下载` : `${n} 次下载`;
};
const fmtSize = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${(b / 1e6).toFixed(0)} MB`);

function OllamaModels() {
  const { tasks, clearTask } = useHub();
  const { confirm, toast } = useUi();
  const [models, setModels] = useState<LocalModel[]>([]);
  const [embeds, setEmbeds] = useState<EmbeddingModelInfo[] | null>(null);
  const [lib, setLib] = useState<{ embedding: LibraryModel[]; chat: LibraryModel[]; offline: boolean } | null>(null);
  const [tab, setTab] = useState<'embedding' | 'chat'>('embedding');

  const load = () => api.listModels().then(setModels).catch(() => undefined);
  const loadLib = async () => {
    setLib(null);
    try {
      const [embedding, chat] = await Promise.all([api.modelLibrary('embedding'), api.modelLibrary('chat')]);
      setLib({ embedding, chat, offline: false });
    } catch {
      // 拿不到 ollama.com 时退回内置清单
      const conv = (list: typeof catalog.chatModels): LibraryModel[] => list.map((m) => ({
        name: m.name.split(':')[0], description: m.note, sizes: m.name.includes(':') ? [m.name.split(':')[1]] : [], capabilities: [], pulls: ''
      }));
      setLib({ embedding: conv(catalog.embeddingModels), chat: conv(catalog.chatModels), offline: true });
    }
  };
  useEffect(() => { load(); loadLib(); }, []);
  useEffect(() => { if (Object.entries(tasks).some(([k, t]) => k.startsWith('model:') && t.done)) load(); }, [tasks]);
  // 已下载模型实测维度（服务台缓存）：能向量化的是向量模型，其余为对话模型
  const loadEmbeds = () => api.embeddingModels().then(setEmbeds).catch(() => setEmbeds([]));
  useEffect(() => { loadEmbeds(); }, [models]);
  const changeDims = async (model: string, value: number, native: number) => {
    const r = await confirm({ level: 'L2', title: `把 ${model} 的维度改为 ${value}？`,
      message: '知舟里已经用这个模型建好的索引会和新维度不一致，改完需要在知舟「知识检索」里重建索引（扫码重新配对也会带上新维度）。',
      confirmText: '修改' });
    if (!r.ok) return;
    await api.setEmbeddingDims(model, value === native ? 0 : value);
    loadEmbeds();
  };

  const installed = (ref: string) => models.some((m) => m.name === ref || m.name === `${ref}:latest`);
  const pull = (ref: string) => { clearTask(`model:${ref}`); api.pullModel(ref).catch((e) => toast(String(e))); };
  const remove = async (name: string) => {
    const r = await confirm({ level: 'L1', title: `删除模型 ${name}？`, message: '模型文件会从磁盘删除，知舟若正在使用它做向量化将无法继续。需要时可重新下载。', confirmText: '删除' });
    if (r.ok) { await api.deleteModel(name); load(); }
  };
  const pulling = Object.entries(tasks).filter(([k, t]) => k.startsWith('model:') && !t.done && !installed(k.slice(6)));

  const card = (m: LibraryModel) => {
    const refs = m.sizes.length ? m.sizes.map((s) => ({ ref: `${m.name}:${s}`, label: s })) : [{ ref: m.name, label: '下载' }];
    return (
      <div className="model" key={m.name}>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
          <span className="model-name" title={m.name}>{m.name}</span>
          <span className="row-meta" style={{ whiteSpace: 'nowrap' }}>{fmtPulls(m.pulls)}</span>
        </div>
        <div className="row-meta" style={{ display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }} title={m.description}>{m.description}</div>
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
          {refs.map(({ ref, label }) => {
            const t = tasks[`model:${ref}`];
            const busy = !!t && !t.done;
            return installed(ref)
              ? <Tag key={ref} tone="ok">{label === '下载' ? '已下载' : `${label} 已下载`}</Tag>
              : <Button key={ref} small busy={busy} title={`下载 ${ref}`} onClick={() => pull(ref)}>{busy ? `${t.percent ?? 0}%` : label}</Button>;
          })}
        </div>
      </div>
    );
  };

  return (
    <div className="row" style={{ flexDirection: 'column', alignItems: 'stretch', gap: 12 }}>
      <div>
        <div className="row-title">已下载的模型</div>
        <div className="row-desc">在知舟「检索与索引 → 向量模型」里填写的维度必须与这里一致。</div>
      </div>
      {models.length === 0 && pulling.length === 0 && <div className="row-meta">还没有模型，从下方模型库挑一个下载。知识库检索推荐中文表现好的向量模型，如 qwen3-embedding 或 bge-m3。</div>}
      {models.map((m) => {
        const e = embeds?.find((x) => x.model === m.name);
        return (
          <div key={m.name} style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
            <span className="model-name" style={{ flex: 1 }}>{m.name}</span>
            <span className="row-meta">{fmtSize(m.sizeBytes)}</span>
            {embeds === null ? <Tag tone="idle">识别中</Tag> : !e ? <Tag tone="idle">对话模型</Tag> : e.options.length > 1 ? (
              <select className="input" style={{ width: 168, height: 32 }} value={e.dims} title="知舟使用的向量维度"
                onChange={(ev) => changeDims(m.name, Number(ev.target.value), e.nativeDims)}>
                {e.options.map((d) => <option key={d} value={d}>{d === e.nativeDims ? `${d} 维（原生）` : `${d} 维`}</option>)}
              </select>
            ) : <Tag tone="brand">向量 · {e.dims} 维</Tag>}
            <Button small kind="text" onClick={() => remove(m.name)}>删除</Button>
          </div>
        );
      })}
      {embeds?.some((e) => e.options.length > 1) && <div className="row-meta">可下拉选择维度的模型支持「截取维度」：维度越小，手机上占用越少、检索越快，效果略降。中文知识库 1024 维通常就够用。</div>}
      {pulling.map(([k, t]) => (
        <div key={k}><div className="model-name" style={{ marginBottom: 6 }}>{k.slice(6)}</div><TaskBar task={t} /></div>
      ))}

      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: 8 }}>
        <Segmented value={tab} onChange={setTab} options={[{ value: 'embedding', label: '向量模型 · 最新' }, { value: 'chat', label: '对话模型 · 热门' }]} />
        <Button small kind="text" onClick={loadLib}>刷新</Button>
      </div>
      {!lib && <div className="row-meta"><span className="spin" />　正在从 ollama.com 获取模型库…</div>}
      {lib?.offline && <div className="row-meta">连不上 ollama.com，显示的是内置推荐清单。</div>}
      {lib && <div className="models">{lib[tab].map(card)}</div>}
    </div>
  );
}
