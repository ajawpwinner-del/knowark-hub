// 设计组件：按知舟 operation-contract —— 按钮四级、唯一确认弹窗（L1 删除 / L2 覆盖 / L4 隐私）、toast 白名单出口
import { createContext, ReactNode, useCallback, useContext, useRef, useState } from 'react';

type BtnKind = 'emph' | 'normal' | 'warn' | 'text';
export function Button(p: { kind?: BtnKind; small?: boolean; disabled?: boolean; busy?: boolean; onClick?: () => void; children: ReactNode; title?: string }) {
  return (
    <button className={`btn ${p.kind ?? 'normal'}${p.small ? ' sm' : ''}`} disabled={p.disabled || p.busy} onClick={p.onClick} title={p.title}>
      {p.busy && <span className="spin" />}
      {p.children}
    </button>
  );
}

export function Group(p: { title?: ReactNode; extra?: ReactNode; pad?: boolean; children: ReactNode }) {
  return (
    <section className="group">
      {(p.title || p.extra) && <div className="group-title"><span>{p.title}</span>{p.extra}</div>}
      <div className={`card${p.pad ? ' pad' : ''}`}>{p.children}</div>
    </section>
  );
}

export function Tag(p: { tone: 'ok' | 'warn' | 'bad' | 'idle' | 'brand'; busy?: boolean; children: ReactNode }) {
  return <span className={`tag ${p.tone}${p.busy ? ' busy' : ''}`}>{p.children}</span>;
}

export function Segmented<T extends string>(p: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <div className="seg" role="radiogroup">
      {p.options.map((o) => (
        <button key={o.value} className={o.value === p.value ? 'on' : ''} role="radio" aria-checked={o.value === p.value} onClick={() => p.onChange(o.value)}>{o.label}</button>
      ))}
    </div>
  );
}

export function Switch(p: { on: boolean; onChange: (v: boolean) => void; label?: string }) {
  return <button className={`switch${p.on ? ' on' : ''}`} role="switch" aria-checked={p.on} aria-label={p.label} onClick={() => p.onChange(!p.on)} />;
}

// ---------- 确认弹窗与 toast ----------
export interface ConfirmOptions {
  level: 'L1' | 'L2' | 'L4';
  title: string;
  message: ReactNode;
  confirmText: string; // 动词，不用「确定」
  checkbox?: string;
}
interface UiApi {
  confirm: (o: ConfirmOptions) => Promise<{ ok: boolean; checked: boolean }>;
  toast: (text: string) => void;
}
const UiCtx = createContext<UiApi>({ confirm: async () => ({ ok: false, checked: false }), toast: () => undefined });
export const useUi = () => useContext(UiCtx);

export function UiProvider({ children }: { children: ReactNode }) {
  const [dlg, setDlg] = useState<(ConfirmOptions & { resolve: (r: { ok: boolean; checked: boolean }) => void }) | null>(null);
  const [checked, setChecked] = useState(false);
  const [toastText, setToastText] = useState<string | null>(null);

  const confirm = useCallback((o: ConfirmOptions) => new Promise<{ ok: boolean; checked: boolean }>((resolve) => {
    setChecked(false);
    setDlg({ ...o, resolve });
  }), []);
  const timer = useRef<number>();
  const toast = useCallback((text: string) => {
    setToastText(text);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setToastText(null), 2500);
  }, []);
  const close = (ok: boolean) => { dlg?.resolve({ ok, checked }); setDlg(null); };

  return (
    <UiCtx.Provider value={{ confirm, toast }}>
      {children}
      {dlg && (
        <div className="scrim" onClick={() => close(false)}>
          <div className="dialog" role="alertdialog" onClick={(e) => e.stopPropagation()}>
            <h3>{dlg.title}</h3>
            <p>{dlg.message}</p>
            {dlg.checkbox && (
              <label className="check"><input type="checkbox" checked={checked} onChange={(e) => setChecked(e.target.checked)} />{dlg.checkbox}</label>
            )}
            <div className="dialog-actions">
              <Button onClick={() => close(false)}>取消</Button>
              <Button kind={dlg.level === 'L4' ? 'emph' : 'warn'} onClick={() => close(true)}>{dlg.confirmText}</Button>
            </div>
          </div>
        </div>
      )}
      {toastText && <div className="toast" role="status">{toastText}</div>}
    </UiCtx.Provider>
  );
}

// ---------- 图标：单线等粗、平切笔端，呼应「黑金一笔」 ----------
const P = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.7, strokeLinecap: 'butt' as const, strokeLinejoin: 'round' as const };
export const Icons = {
  home: <svg viewBox="0 0 24 24" {...P}><path d="M4 10.5 12 4l8 6.5V20h-5v-6H9v6H4z" /></svg>,
  grid: <svg viewBox="0 0 24 24" {...P}><rect x="4" y="4" width="7" height="7" rx="2" /><rect x="13" y="4" width="7" height="7" rx="2" /><rect x="4" y="13" width="7" height="7" rx="2" /><path d="M16.5 13.5v6M13.5 16.5h6" /></svg>,
  server: <svg viewBox="0 0 24 24" {...P}><rect x="4" y="4" width="16" height="7" rx="2" /><rect x="4" y="13" width="16" height="7" rx="2" /><path d="M8 7.5h.01M8 16.5h.01M12 7.5h5M12 16.5h5" /></svg>,
  link: <svg viewBox="0 0 24 24" {...P}><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" /></svg>,
  gear: <svg viewBox="0 0 24 24" {...P}><circle cx="12" cy="12" r="3" /><path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8" /></svg>,
  model: <svg viewBox="0 0 24 24" {...P}><circle cx="12" cy="12" r="3" /><circle cx="5" cy="6" r="1.8" /><circle cx="19" cy="6" r="1.8" /><circle cx="5" cy="18" r="1.8" /><circle cx="19" cy="18" r="1.8" /><path d="m6.5 7 3.3 3M17.5 7l-3.3 3M6.5 17l3.3-3M17.5 17l-3.3-3" /></svg>,
  search: <svg viewBox="0 0 24 24" {...P}><circle cx="11" cy="11" r="6" /><path d="m15.5 15.5 4.5 4.5" /></svg>,
  ocr: <svg viewBox="0 0 24 24" {...P}><path d="M4 8V5.5A1.5 1.5 0 0 1 5.5 4H8M16 4h2.5A1.5 1.5 0 0 1 20 5.5V8M20 16v2.5a1.5 1.5 0 0 1-1.5 1.5H16M8 20H5.5A1.5 1.5 0 0 1 4 18.5V16M8 9h8M8 12h8M8 15h5" /></svg>,
  doc: <svg viewBox="0 0 24 24" {...P}><path d="M14 3.5H7.5a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2V8z" /><path d="M14 3.5V8h4.5M9 12.5h6M9 15.5h6" /></svg>,
  copy: <svg viewBox="0 0 24 24" {...P}><rect x="8" y="8" width="12" height="12" rx="2.5" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></svg>
};
export const serviceIcon = (category: string) =>
  ({ model: Icons.model, search: Icons.search, ocr: Icons.ocr, doc: Icons.doc } as Record<string, JSX.Element>)[category] ?? Icons.server;
