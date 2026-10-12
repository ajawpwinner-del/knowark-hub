import { useEffect, useMemo, useState } from 'react';
import QRCode from 'qrcode';
import { api, ConnectInfo, EmbeddingModelInfo, saveTextFile } from '../api';
import { Button, Group, Icons, Tag, useUi } from '../components/design';
import { useHub } from '../store';

interface Field { k: string; v: string; secret?: boolean }
interface Entry { id: string; title: string; where: string; fields: Field[] }

export function Connect() {
  const { services } = useHub();
  const { confirm, toast } = useUi();
  const [info, setInfo] = useState<ConnectInfo | null>(null);
  const [ip, setIp] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [qr, setQr] = useState('');
  const [embeds, setEmbeds] = useState<EmbeddingModelInfo[]>([]);
  useEffect(() => { api.embeddingModels().then(setEmbeds).catch(() => setEmbeds([])); }, [services]);
  const load = () => api.connectInfo().then((i) => { setInfo(i); setIp((cur) => cur || i.addresses[0]?.ip || '127.0.0.1'); });
  useEffect(() => { load(); }, []);

  const base = info ? `http://${ip}:${info.gatewayPort}` : '';
  const running = (id: string) => services.some((s) => s.id === id && s.status === 'running');
  const key = info?.key ?? '';

  const entries: Entry[] = useMemo(() => {
    const list: Entry[] = [];
    if (running('mineru')) {
      list.push({
        id: 'doc', title: '文档解析', where: '知舟设置 → 文档解析',
        fields: [{ k: '服务', v: 'MinerU 本地部署' }, { k: '地址', v: base }, { k: '模型', v: 'standard' }, { k: 'Key', v: key, secret: true }]
      });
    }
    if (running('searxng')) {
      list.push({
        id: 'search', title: '网络搜索', where: '知舟设置 → 网络搜索 → SearXNG',
        fields: [{ k: '服务地址', v: base }, { k: '用户名', v: 'knowark' }, { k: '密码', v: key, secret: true }]
      });
    }
    if (running('ollama')) {
      for (const m of embeds) {
        list.push({
          id: 'embed-' + m.model, title: '向量模型 · ' + m.model, where: '知舟设置 → 知识检索 → 向量模型',
          fields: [{ k: '协议', v: 'OpenAI 兼容' }, { k: 'Base URL', v: `${base}/v1` }, { k: '模型', v: m.model }, { k: '维度', v: '服务商默认（' + m.dims + ' 维）' }, { k: 'API Key', v: key, secret: true }]
        });
      }
      list.push({
        id: 'chat', title: '本地对话模型（可选）', where: '知舟设置 → 模型服务 → 添加',
        fields: [{ k: '协议', v: 'OpenAI 兼容' }, { k: 'Base URL', v: `${base}/v1` }, { k: '模型', v: '在知舟中点「获取模型列表」' }, { k: 'API Key', v: key, secret: true }]
      });
    }
    return list;
  }, [services, base, key, embeds]);

  const config = useMemo(() => JSON.stringify({
    kind: 'knowark-hub', v: 1, gateway: base, key,
    services: Object.fromEntries(entries.map((e) => [e.id, Object.fromEntries(e.fields.filter((f) => !f.secret).map((f) => [f.k, f.v]))]))
  }, null, 2), [entries, base, key]);

  useEffect(() => {
    if (!base) return;
    QRCode.toDataURL(JSON.stringify({ kind: 'knowark-hub', v: 1, gateway: base, key }), { margin: 0, width: 272, color: { dark: '#1E2229', light: '#FFFFFF' } }).then(setQr);
  }, [base, key]);

  const copy = async (text: string) => {
    try { await navigator.clipboard.writeText(text); toast('已复制'); } catch { toast('复制失败'); }
  };

  const rotate = async () => {
    const r = await confirm({ level: 'L2', title: '更换 Key？', message: '旧 Key 立即失效，手机上的知舟需要把所有服务的 Key 改成新值后才能继续使用。', confirmText: '更换' });
    if (r.ok) { await api.rotateKey(); await load(); toast('已更换'); }
  };

  const firewall = async () => {
    const r = await confirm({
      level: 'L4', title: '允许手机访问服务台？',
      message: `将在 Windows 防火墙中为端口 ${info?.gatewayPort} 添加入站规则，仅对「专用网络」（家庭/办公 Wi‑Fi）生效。接下来系统会请求管理员授权。`,
      confirmText: '添加规则'
    });
    if (!r.ok) return;
    try { await api.addFirewallRule(); await load(); toast('已添加'); } catch (e) { toast(String(e)); }
  };

  if (!info) return <div className="page"><span className="spin" /></div>;

  return (
    <div className="page">
      <h1 className="page-title">连接知舟</h1>
      <p className="page-sub">手机与电脑连同一个 Wi‑Fi，在知舟「设置 → 连接服务台」点「扫码配对」扫下面的二维码，一次配好。也可以照下方的值手动填写。</p>

      {info.gatewayError && <div className="notice danger"><span>⚠</span><div><b>网关没有启动：</b>{info.gatewayError}。可在「设置」中换一个端口。</div></div>}
      {!info.firewallReady && (
        <div className="notice">
          <span>ℹ</span>
          <div style={{ flex: 1 }}><b>手机可能连不上：</b>Windows 防火墙默认会拦截来自局域网的访问。</div>
          <Button small kind="emph" onClick={firewall}>允许访问</Button>
        </div>
      )}

      <Group title="扫码配对">
        <div className="row connect-head">
          {qr && <img className="qr" src={qr} alt="连接二维码" />}
          <div className="row-main">
            <div className="kv">
              <span className="kv-k">网络</span>
              <select className="input" style={{ height: 36 }} value={ip} onChange={(e) => setIp(e.target.value)}>
                {info.addresses.map((a) => <option key={a.ip} value={a.ip}>{a.ip}（{a.adapter}）</option>)}
                <option value="127.0.0.1">127.0.0.1（仅本机）</option>
              </select>
              <span />
            </div>
            <div className="kv"><span className="kv-k">地址</span><span className="kv-v">{base}</span><Button small kind="text" onClick={() => copy(base)}>复制</Button></div>
            <div className="kv">
              <span className="kv-k">Key</span>
              <span className="kv-v">{showKey ? key : `${key.slice(0, 6)}••••••••••••${key.slice(-4)}`}</span>
              <span style={{ display: 'flex' }}>
                <Button small kind="text" onClick={() => setShowKey(!showKey)}>{showKey ? '隐藏' : '显示'}</Button>
                <Button small kind="text" onClick={() => copy(key)}>复制</Button>
              </span>
            </div>
            <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
              <Button small onClick={() => saveTextFile('knowark-hub.json', config).then((ok) => ok && toast('已保存'))}>导出配置文件</Button>
              <Button small kind="text" onClick={rotate}>更换 Key</Button>
            </div>
          </div>
        </div>
      </Group>

      {entries.length === 0 && <div className="card empty">还没有运行中的服务。安装并启动后，这里会列出知舟的填写方式。</div>}
      {entries.map((e) => (
        <Group key={e.id} title={e.title} extra={<span className="row-meta">{e.where}</span>}>
          {e.fields.map((f) => (
            <div className="kv" key={f.k}>
              <span className="kv-k">{f.k}</span>
              <span className="kv-v">{f.secret && !showKey ? '与上方 Key 相同' : f.v}</span>
              <button className="btn text sm" title="复制" onClick={() => copy(f.v)} style={{ width: 32, padding: 0 }}><span style={{ width: 18, height: 18, display: 'inline-flex' }}>{Icons.copy}</span></button>
            </div>
          ))}
        </Group>
      ))}
      <p className="row-meta">地址末尾的 <code>#</code> 表示知舟按原样使用该地址、不再自动补路径。<Tag tone="brand">提示</Tag> 二维码与配置文件含 Key，请勿发给他人。</p>
    </div>
  );
}
