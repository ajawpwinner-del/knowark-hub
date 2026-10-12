// 自绘标题栏：无边框窗口下与书院底色一体；空白处可拖动、双击最大化
import { useEffect, useState } from 'react';
import { inTauri } from '../api';

async function win() {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}

const S = { fill: 'none', stroke: 'currentColor', strokeWidth: 1 };

export function TitleBar() {
  const [max, setMax] = useState(false);
  useEffect(() => {
    if (!inTauri) return;
    let off: (() => void) | undefined;
    win().then(async (w) => {
      setMax(await w.isMaximized());
      off = await w.onResized(async () => setMax(await w.isMaximized()));
    });
    return () => off?.();
  }, []);
  if (!inTauri) return null;

  return (
    <div className="titlebar" data-tauri-drag-region onDoubleClick={() => win().then((w) => w.toggleMaximize())}>
      <div className="titlebar-controls" onDoubleClick={(e) => e.stopPropagation()}>
        <button aria-label="最小化" onClick={() => win().then((w) => w.minimize())}>
          <svg viewBox="0 0 10 10"><path d="M0 5.5h10" {...S} /></svg>
        </button>
        <button aria-label={max ? '还原' : '最大化'} onClick={() => win().then((w) => w.toggleMaximize())}>
          {max
            ? <svg viewBox="0 0 10 10"><rect x="0.5" y="2.5" width="7" height="7" {...S} /><path d="M2.5 2.5V.5h7v7h-2" {...S} /></svg>
            : <svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" {...S} /></svg>}
        </button>
        <button className="close" aria-label="关闭" onClick={() => win().then((w) => w.close())}>
          <svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0 0 10" {...S} /></svg>
        </button>
      </div>
    </div>
  );
}
