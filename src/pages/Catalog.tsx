import { api, catalog } from '../api';
import { Button, Tag, serviceIcon, useUi } from '../components/design';
import { statusTag, TaskView, useHub } from '../store';
import type { Page } from '../App';

export function TaskBar({ task }: { task?: TaskView }) {
  if (!task || (task.done && !task.error)) return null;
  if (task.error) return <div className="row-desc" style={{ color: 'var(--danger-primary)' }}>失败：{task.error}</div>;
  return (
    <div>
      <div className={`progress${task.percent === undefined ? ' indet' : ''}`}><i style={task.percent !== undefined ? { width: `${task.percent}%` } : undefined} /></div>
      <div className="progress-line">{task.percent !== undefined ? `${task.percent}% · ` : ''}{task.line}</div>
    </div>
  );
}

export function Catalog({ go }: { go: (p: Page) => void }) {
  const { services, tasks, env, settings, saveSettings, clearTask } = useHub();
  const { toast } = useUi();

  const install = async (id: string) => {
    clearTask(id);
    try { await api.install(id); } catch (e) { toast(`无法开始安装：${e}`); }
  };

  return (
    <div className="page">
      <h1 className="page-title">服务目录</h1>
      <p className="page-sub">每项服务都运行在独立容器里，随时可停用或卸载，不影响电脑上的其它软件。</p>
      <div className="svc-grid">
        {catalog.services.map((d) => {
          const st = services.find((s) => s.id === d.id);
          const task = tasks[d.id];
          const busy = !!task && !task.done;
          const tag = statusTag(st?.status, busy);
          const installed = st && st.status !== 'not_installed';
          const gpuOn = settings?.services[d.id]?.useGpu ?? false;
          return (
            <article className="svc" key={d.id}>
              <div className="svc-head">
                <div className="svc-icon">{serviceIcon(d.category)}</div>
                <div style={{ flex: 1 }}>
                  <div className="svc-name">{d.name}</div>
                  <div className="svc-engine">{d.engine}</div>
                </div>
                <Tag tone={tag.tone} busy={busy}>{tag.text}</Tag>
              </div>
              <div className="svc-summary">{d.summary}</div>
              <div className="svc-needs">
                <span className="chip">内存 ≥ {d.needs.ramGb} GB</span>
                <span className="chip">磁盘约 {d.needs.diskGb} GB</span>
                {d.gpu !== 'none' && <span className="chip">{gpuOn ? '使用显卡加速' : '可用显卡加速'}</span>}
              </div>
              <div className="row-meta">{d.needs.note}</div>
              <TaskBar task={task} />
              <div className="svc-foot">
                {d.gpu !== 'none' && env?.hasNvidia && !installed && settings ? (
                  <Button kind="text" small onClick={() => saveSettings({ ...settings, services: { ...settings.services, [d.id]: { ...settings.services[d.id], useGpu: !gpuOn } } })}>
                    {gpuOn ? '改用 CPU' : '改用显卡'}
                  </Button>
                ) : <span />}
                {installed
                  ? <Button onClick={() => go('services')}>管理</Button>
                  : <Button kind="emph" busy={busy} disabled={env?.dockerReady === false} onClick={() => install(d.id)}>{busy ? '安装中' : '安装'}</Button>}
              </div>
            </article>
          );
        })}
      </div>
      {env?.dockerReady === false && <p className="row-meta" style={{ marginTop: 16 }}>Docker Desktop 未就绪，安装按钮暂不可用：请先完成首页的环境体检。</p>}
    </div>
  );
}
