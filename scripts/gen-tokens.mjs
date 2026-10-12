// 知舟服务台的设计令牌生成器：色值唯一来源是 docs/brand/tools/brand-academy.mjs（书院）与 entry 的 color.json（状态色），
// 这里只做格式转换（HarmonyOS 的 #AARRGGBB → CSS rgba），不另抄色值。品牌改色后重跑本脚本即可同步。
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const hub = resolve(here, '..');
const repo = resolve(hub, '../..');

const { TOKENS, THEME } = await import(pathToFileURL(resolve(repo, 'docs/brand/tools/brand-academy.mjs')).href);

const STATUS = ['danger_primary', 'success_primary', 'warning_primary', 'fill_tonal', 'danger_tonal', 'divider', 'scrim',
  'shadow_card', 'interaction_focus', 'interaction_touch'];

function readColorJson(mode) {
  const json = JSON.parse(readFileSync(resolve(repo, `entry/src/main/resources/${mode}/element/color.json`), 'utf8'));
  return new Map(json.color.map((c) => [c.name, c.value]));
}

// #RRGGBB 或 #AARRGGBB → CSS 颜色
function toCss(hex) {
  const h = hex.replace('#', '');
  if (h.length === 6) return `#${h}`;
  const a = parseInt(h.slice(0, 2), 16) / 255;
  const r = parseInt(h.slice(2, 4), 16), g = parseInt(h.slice(4, 6), 16), b = parseInt(h.slice(6, 8), 16);
  return a >= 0.999 ? `#${h.slice(2)}` : `rgba(${r}, ${g}, ${b}, ${a.toFixed(3)})`;
}

const cssName = (n) => `--${n.replace(/_/g, '-')}`;

function block(mode) {
  const lines = [];
  for (const t of TOKENS) lines.push(`  ${cssName(t.name)}: ${toCss(t[mode])};`);
  for (const t of THEME) lines.push(`  ${cssName(t.name)}: ${toCss(t[mode])};`);
  const status = readColorJson(mode === 'light' ? 'base' : 'dark');
  for (const n of STATUS) {
    const v = status.get(n);
    if (!v) throw new Error(`color.json 缺少 ${n}（${mode}）`);
    lines.push(`  ${cssName(n)}: ${toCss(v)};`);
  }
  return lines.join('\n');
}

const out = `/* 自动生成：node scripts/gen-tokens.mjs —— 来源 docs/brand/tools/brand-academy.mjs 与 entry color.json，勿手改 */
:root {
${block('light')}
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
${block('dark').replace(/^/gm, '  ')}
    color-scheme: dark;
  }
}
:root[data-theme="dark"] {
${block('dark')}
  color-scheme: dark;
}
`;

mkdirSync(resolve(hub, 'src/styles'), { recursive: true });
writeFileSync(resolve(hub, 'src/styles/tokens.css'), out);

// 品牌标志与字标：直接复用 entry 资源
mkdirSync(resolve(hub, 'public'), { recursive: true });
for (const f of ['brand_mark.svg', 'brand_wordmark.svg', 'brand_seal.svg']) {
  copyFileSync(resolve(repo, 'entry/src/main/resources/base/media', f), resolve(hub, 'public', f));
}
console.log(`令牌已生成：${TOKENS.length + THEME.length + STATUS.length} 项 × 浅/深`);
