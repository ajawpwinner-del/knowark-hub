import { catalog, openUrl } from '../api';
import { Button, Group, Tag, serviceIcon } from '../components/design';
import { statusTag, useHub } from '../store';
import type { Page } from '../App';

export function Home({ go }: { go: (p: Page) => void }) {
  const { env, services, refreshEnv } = useHub();
  const running = services.filter((s) => s.status === 'running').length;
  const blocking = env?.items.filter((i) => i.status === 'missing') ?? [];

  return (
    <div className="page">
      <section className="hero">
        <img className="hero-mark" src="/brand_mark.svg" alt="" />
        <div>
          <h2>{running > 0 ? `${running} 项服务正在为知舟工作` : '把文档解析、搜索与本地模型装进自己的电脑'}</h2>
          <p>{env?.dockerReady === false
            ? '先按下方体检结果准备好 Docker Desktop，之后每项服务都只需点一下「安装」。'
            : '挑选需要的服务一键部署，再到「连接知舟」用手机上的知舟扫码配对即可。'}</p>
          <Button kind="emph" onClick={() => go(running > 0 ? 'connect' : 'catalog')}>{running > 0 ? '连接知舟' : '去挑选服务'}</Button>
        </div>
      </section>

      <div className="stat-row">
        {catalog.services.map((d) => {
          const st = services.find((s) => s.id === d.id);
          const tag = statusTag(st?.status);
          return (
            <div key={d.id} className="stat" onClick={() => go('services')}>
              <div className="stat-label"><span style={{ width: 18, height: 18, display: 'inline-flex', color: 'var(--brand-primary)' }}>{serviceIcon(d.category)}</span>{d.name}</div>
              <div className="stat-value"><Tag tone={tag.tone}>{tag.text}</Tag></div>
            </div>
          );
        })}
      </div>

      {blocking.length > 0 && (
        <div className="notice danger">
          <span>⚠</span>
          <div><b>还差 {blocking.length} 步就能开始部署。</b>按下方提示完成后点「重新检测」。</div>
        </div>
      )}

      <Group title="环境体检" extra={<Button kind="text" small onClick={refreshEnv}>重新检测</Button>}>
        {!env && <div className="row"><span className="spin" />　正在检测…</div>}
        {env?.items.map((i) => (
          <div className="row" key={i.id}>
            <span className={`env-icon ${i.status}`}>{i.status === 'ok' ? '✓' : i.status === 'warn' ? '!' : '×'}</span>
            <div className="row-main">
              <div className="row-title">{i.label}</div>
              <div className="row-desc">{i.detail}</div>
              {i.id === 'docker' && i.status === 'missing' && (
                <ol className="steps">
                  <li>点右侧按钮下载 Docker Desktop，一路「下一步」安装（勾选使用 WSL 2）</li>
                  <li>安装完成后重启电脑，打开 Docker Desktop，同意条款，可跳过登录</li>
                  <li>等待左下角出现绿色「Engine running」，回到这里点「重新检测」</li>
                </ol>
              )}
            </div>
            {i.actionUrl && <Button small onClick={() => openUrl(i.actionUrl!)}>{i.actionLabel}</Button>}
          </div>
        ))}
      </Group>
    </div>
  );
}
