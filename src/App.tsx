import { useState } from 'react';
import { Icons } from './components/design';
import { TitleBar } from './components/TitleBar';
import { useHub } from './store';
import { Home } from './pages/Home';
import { Catalog } from './pages/Catalog';
import { Services } from './pages/Services';
import { Connect } from './pages/Connect';
import { Settings } from './pages/Settings';
import { inTauri } from './api';

export type Page = 'home' | 'catalog' | 'services' | 'connect' | 'settings';

const NAV: { id: Page; label: string; icon: JSX.Element }[] = [
  { id: 'home', label: '首页', icon: Icons.home },
  { id: 'catalog', label: '服务目录', icon: Icons.grid },
  { id: 'services', label: '我的服务', icon: Icons.server },
  { id: 'connect', label: '连接知舟', icon: Icons.link },
  { id: 'settings', label: '设置', icon: Icons.gear }
];

export function App() {
  const [page, setPage] = useState<Page>('home');
  const { services, env } = useHub();
  const running = services.filter((s) => s.status === 'running').length;

  return (
    <div className={`shell${inTauri ? ' has-titlebar' : ''}`}>
      <TitleBar />
      <nav className="nav" data-tauri-drag-region>
        <div className="brand">
          <img src="/brand_mark.svg" alt="" />
          <div><div className="brand-name">知舟</div><div className="brand-sub">服务台 · HUB</div></div>
        </div>
        {NAV.map((n) => (
          <button key={n.id} className={`nav-item${page === n.id ? ' active' : ''}`} onClick={() => setPage(n.id)}>
            {n.icon}{n.label}
          </button>
        ))}
        <div className="nav-spacer" />
        <div className="nav-foot">
          <span className={`tag ${env?.dockerReady ? 'ok' : 'warn'}`}>{env?.dockerReady ? 'Docker 就绪' : 'Docker 未就绪'}</span>
          <span>{running} 项运行</span>
        </div>
        {!inTauri && <div className="nav-foot">浏览器预览 · 演示数据</div>}
      </nav>
      <main className="main" key={page}>
        {page === 'home' && <Home go={setPage} />}
        {page === 'catalog' && <Catalog go={setPage} />}
        {page === 'services' && <Services go={setPage} />}
        {page === 'connect' && <Connect />}
        {page === 'settings' && <Settings />}
      </main>
    </div>
  );
}
