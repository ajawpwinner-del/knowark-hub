// 服务面板的「设置 / 试一试 / 使用说明」页签
import { useState } from 'react';
import { api, openUrl, ParseResult, pickFile, SearchHit, ServiceDef } from '../api';
import { Button, Segmented, Switch, Tag, useUi } from '../components/design';
import { guides } from '../catalog/guides';
import { useHub } from '../store';

// ---------- 设置 ----------
interface Opt { key: string; label: string; desc?: string; def: string; kind: 'seg' | 'switch' | 'multi'; options?: { value: string; label: string }[] }

const SEARXNG_ENGINES = [
  { value: 'bing', label: '必应' }, { value: 'baidu', label: '百度' }, { value: 'sogou', label: '搜狗' },
  { value: '360search', label: '360 搜索' }, { value: 'wikipedia', label: '维基百科' }, { value: 'duckduckgo', label: 'DuckDuckGo' },
  { value: 'google', label: 'Google' }, { value: 'brave', label: 'Brave' }, { value: 'github', label: 'GitHub' }, { value: 'arxiv', label: 'arXiv 论文' }
];

const OPTIONS: Record<string, Opt[]> = {
  searxng: [
    { key: 'engines', label: '使用的搜索引擎', desc: '国内网络建议以必应、百度、搜狗为主；开着代理时可加上 Google。选得越多结果越全，但会慢一点。', def: 'bing,baidu,sogou,360search,wikipedia,duckduckgo', kind: 'multi', options: SEARXNG_ENGINES },
    { key: 'language', label: '结果语言', def: 'zh-CN', kind: 'seg', options: [{ value: 'zh-CN', label: '中文' }, { value: 'en', label: '英文' }, { value: 'all', label: '不限' }] },
    { key: 'safe_search', label: '安全搜索', desc: '过滤成人内容。', def: '0', kind: 'seg', options: [{ value: '0', label: '关闭' }, { value: '1', label: '适中' }, { value: '2', label: '严格' }] }
  ],
  mineru: [
    { key: 'lang', label: '文档主要语言', desc: '影响扫描页的文字识别。中英混排选中文。', def: 'ch', kind: 'seg', options: [{ value: 'ch', label: '中文' }, { value: 'en', label: '英文' }, { value: 'japan', label: '日文' }, { value: 'korean', label: '韩文' }] },
    { key: 'formula', label: '识别数学公式', desc: '转成 LaTeX。关闭可加快速度。', def: 'true', kind: 'switch' },
    { key: 'table', label: '识别表格', desc: '转成 Markdown 表格。关闭可加快速度。', def: 'true', kind: 'switch' }
  ]
};

export function SettingsTab({ def }: { def: ServiceDef }) {
  const { settings, saveSettings } = useHub();
  const { toast } = useUi();
  const opts = OPTIONS[def.id];
  const saved = settings?.services[def.id]?.options ?? {};
  const [draft, setDraft] = useState<Record<string, string>>(saved);
  const [busy, setBusy] = useState(false);
  if (!settings) return null;
  if (!opts) {
    return <div className="row"><div className="row-desc">{def.id === 'ollama' ? '模型在「概览」页的模型库里下载与管理；端口与镜像在「概览 → 端口与镜像」中修改。' : '这个服务没有需要调整的选项；端口与镜像在「概览 → 端口与镜像」中修改。'}</div></div>;
  }
  const val = (o: Opt) => draft[o.key] ?? o.def;
  const set = (k: string, v: string) => setDraft({ ...draft, [k]: v });
  const dirty = opts.some((o) => (draft[o.key] ?? o.def) !== (saved[o.key] ?? o.def));

  const save = async () => {
    if (def.id === 'searxng' && !val(opts[0])) { toast('至少选择一个搜索引擎'); return; }
    setBusy(true);
    try {
      const cur = settings.services[def.id] ?? { useGpu: false };
      await saveSettings({ ...settings, services: { ...settings.services, [def.id]: { ...cur, options: draft } } });
      await api.applyOptions(def.id);
      toast('已保存');
    } catch (e) { toast(String(e)); } finally { setBusy(false); }
  };

  return (
    <>
      {opts.map((o) => (
        <div className="row" key={o.key} style={o.kind === 'multi' ? { flexDirection: 'column', alignItems: 'stretch' } : undefined}>
          <div className="row-main">
            <div className="row-title">{o.label}</div>
            {o.desc && <div className="row-desc">{o.desc}</div>}
          </div>
          {o.kind === 'seg' && <Segmented value={val(o)} options={o.options!} onChange={(v) => set(o.key, v)} />}
          {o.kind === 'switch' && <Switch on={val(o) === 'true'} label={o.label} onChange={(v) => set(o.key, String(v))} />}
          {o.kind === 'multi' && (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 10 }}>
              {o.options!.map((c) => {
                const list = val(o).split(',').filter(Boolean);
                const on = list.includes(c.value);
                return (
                  <button key={c.value} className={`btn sm ${on ? 'emph' : 'normal'}`} onClick={() => set(o.key, (on ? list.filter((x) => x !== c.value) : [...list, c.value]).join(','))}>
                    {on ? '✓ ' : ''}{c.label}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      ))}
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        {def.id === 'searxng' && <span className="row-meta" style={{ flex: 1 }}>保存后网络搜索会自动重启，约 10 秒。</span>}
        <Button kind="emph" busy={busy} disabled={!dirty} onClick={save}>{busy ? '保存中' : '保存'}</Button>
      </div>
    </>
  );
}

// ---------- 试一试 ----------
export function TryTab({ def, running }: { def: ServiceDef; running: boolean }) {
  if (!running) return <div className="row"><div className="row-desc">服务启动后才能试用。</div></div>;
  if (def.id === 'searxng') return <TrySearch />;
  if (def.id === 'mineru') return <TryParse />;
  return <div className="row"><div className="row-desc">在「概览」页下载模型后，模型旁会显示是否能正常向量化及其维度，这就是最直接的试用结果。</div></div>;
}

function TrySearch() {
  const [q, setQ] = useState('知舟 知识库');
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState('');
  const go = async () => {
    setBusy(true); setErr('');
    const t = Date.now();
    try { const r = await api.trySearch(q); setHits(r); if (!r.length) setErr('没有结果：可能是所选搜索引擎在当前网络下连不上，去「设置」里换几个试试。'); else setErr(`用时 ${((Date.now() - t) / 1000).toFixed(1)} 秒`); }
    catch (e) { setHits(null); setErr(String(e)); } finally { setBusy(false); }
  };
  return (
    <div className="row" style={{ flexDirection: 'column', alignItems: 'stretch', gap: 12 }}>
      <div style={{ display: 'flex', gap: 8 }}>
        <input className="input" value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && q.trim() && go()} placeholder="输入要搜索的内容" />
        <Button kind="emph" busy={busy} disabled={!q.trim()} onClick={go}>{busy ? '搜索中' : '搜索'}</Button>
      </div>
      {err && <div className="row-meta">{err}</div>}
      {hits?.map((h, i) => (
        <div key={i} style={{ cursor: 'pointer' }} onClick={() => openUrl(h.url)}>
          <div style={{ color: 'var(--brand-primary)', fontWeight: 500 }}>{h.title || h.url}</div>
          <div className="row-meta" style={{ fontFamily: 'var(--mono)' }}>{h.url.length > 80 ? h.url.slice(0, 80) + '…' : h.url}{h.engine ? ` · ${h.engine}` : ''}</div>
          {h.content && <div className="row-desc">{h.content}</div>}
        </div>
      ))}
    </div>
  );
}

function TryParse() {
  const [file, setFile] = useState<string | null>(null);
  const [res, setRes] = useState<ParseResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState('');
  const run = async () => {
    const f = await pickFile('document');
    if (!f) return;
    setFile(f); setRes(null); setErr(''); setBusy(true);
    try { setRes(await api.tryParse(f, 'auto')); } catch (e) { setErr(String(e)); } finally { setBusy(false); }
  };
  const name = file?.split(/[\\/]/).pop();
  return (
    <div className="row" style={{ flexDirection: 'column', alignItems: 'stretch', gap: 12 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <div className="row-main">
          <div className="row-title">选一个 PDF 文件</div>
          <div className="row-desc">{busy ? `正在处理 ${name}…（PDF 每页约需几秒，首次还要下载模型）` : '走的是知舟实际使用的同一条通路，结果就是导入知舟后的内容。'}</div>
        </div>
        <Button kind="emph" busy={busy} onClick={run}>{busy ? '处理中' : '选择文件'}</Button>
      </div>
      {err && <div className="row-desc" style={{ color: 'var(--danger-primary)' }}>{err}</div>}
      {res && (
        <>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            <Tag tone="ok">完成</Tag>
            <span className="row-meta">{name} · 用时 {(res.ms / 1000).toFixed(1)} 秒{res.pageCount ? ` · ${res.pageCount} 页` : ''} · {res.markdown.length} 字{res.truncated ? '（仅预览前 2 万字）' : ''}</span>
          </div>
          <pre className="logs" style={{ maxHeight: 420 }}>{res.markdown || '（没有识别到文字）'}</pre>
        </>
      )}
    </div>
  );
}

// ---------- 使用说明 ----------
export function GuideTab({ def, go }: { def: ServiceDef; go: () => void }) {
  const g = guides[def.id];
  if (!g) return null;
  return (
    <div className="row guide" style={{ flexDirection: 'column', alignItems: 'stretch', gap: 16 }}>
      <div className="row-desc" style={{ fontSize: 14, lineHeight: '22px' }}>{g.intro}</div>
      <div>
        <div className="row-title" style={{ marginBottom: 8 }}>在知舟里使用</div>
        <ol className="guide-steps">{g.steps.map((s, i) => <li key={i}>{s}</li>)}</ol>
        <Button small onClick={go}>打开「连接知舟」复制地址和 Key</Button>
      </div>
      <div>
        <div className="row-title" style={{ marginBottom: 8 }}>常见问题</div>
        {g.tips.map((t) => (
          <details key={t.q} className="faq">
            <summary>{t.q}</summary>
            <div className="row-desc">{t.a}</div>
          </details>
        ))}
      </div>
    </div>
  );
}
