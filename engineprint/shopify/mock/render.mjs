// Renderiza a secao product-whatsapp.liquid localmente com liquidjs, imitando
// os objetos/filtros da Shopify que a secao usa. Serve para EXERCITAR a
// interface no navegador antes de tocar na loja — nao substitui o preview real.
import { Liquid } from 'liquidjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
const [,, liquidPath, handle, outPath, extra] = process.argv;
const raw = readFileSync(liquidPath, 'utf8');
const src = raw.replace(/({%-?\s*liquid\b)([\s\S]*?)(-?%})/g, (m, a, b, c) => a + b.replace(/^\s*comment\b[\s\S]*?^\s*endcomment\s*$/gm, '') + c);
const data = JSON.parse(readFileSync(resolve(here, 'dados', handle + '.json'), 'utf8'));
const inv = JSON.parse(extra || '{}');   // { "Padrão": 417, ... } inventory_quantity por titulo

// ---- product no formato Liquid ----
const base = (u) => u.replace(/\?.*$/, '').split('/').pop();
const media = data.media.map((m, i) => ({ id: m.id, alt: m.alt, position: i + 1, src: m.src, aspect_ratio: m.aspect_ratio, width: m.width, height: m.height, media_type: 'image', preview_image: { aspect_ratio: m.aspect_ratio } }));
const variants = data.variants.map((v) => ({
  id: v.id, title: v.title, price: v.price, available: v.available, option1: v.option1, option2: v.option2, option3: v.option3,
  options: [v.option1, v.option2, v.option3].filter((x) => x != null),
  featured_media: v.featured_media ? { position: v.featured_media.position, id: v.featured_media.id } : null,
  inventory_quantity: inv[v.title] ?? 0, inventory_management: 'shopify', inventory_policy: 'deny',
}));
const options_with_values = data.options.map((o, i) => ({ name: o.name, position: i + 1, values: o.values }));
const product = {
  id: data.id, title: data.title, vendor: data.vendor, handle: data.handle, description: data.description,
  media, featured_media: media[0] || null, variants, options: data.options.map((o) => o.name), options_with_values,
  has_only_default_variant: variants.length === 1 && variants[0].title === 'Default Title',
  selected_variant: null,
  selected_or_first_available_variant: variants.find((v) => v.available) || variants[0],
  metafields: { custom: {} },
};
// ---- section.settings a partir do schema ----
const schema = JSON.parse(src.match(/{%\s*schema\s*%}([\s\S]*?){%\s*endschema\s*%}/)[1]);
const settings = {};
for (const s of schema.settings) if ('default' in s) settings[s.id] = s.default;
Object.assign(settings, { warranty_link: '/pages/garantia' });
const section = { id: 'product-whatsapp', settings };

// ---- engine ----
const engine = new Liquid({ strictFilters: false, strictVariables: false, lenientIf: true, jsTruthy: false });
const attrs = (o) => Object.entries(o).filter(([, v]) => v != null && v !== '').map(([k, v]) => ` ${k.replace(/_/g, '-')}="${String(v).replace(/"/g, '&quot;')}"`).join('');
// image_url: aceita media (objeto) ou string; devolve um "url" com a largura pedida
engine.registerFilter('image_url', (m, ...kv) => {
  const o = Object.fromEntries(kv.map((p) => Array.isArray(p) ? p : [p, true]));
  const s = typeof m === 'string' ? m : (m && m.src) || '';
  return { __img: true, src: 'media/' + base(s), width: o.width || 1200, alt: (m && m.alt) || '', aspect: (m && m.aspect_ratio) || 1 };
});
engine.registerFilter('image_tag', (u, ...kv) => {
  const o = Object.fromEntries(kv.map((p) => Array.isArray(p) ? p : [p, true]));
  const src = u && u.__img ? u.src : String(u);
  const w = u && u.__img ? u.width : 1200; const h = Math.round(w / ((u && u.aspect) || 1));
  const { widths, sizes, ...rest } = o;
  return `<img src="${src}" width="${w}" height="${h}"${attrs({ ...rest, alt: rest.alt ?? (u && u.alt) ?? '' })}${sizes ? ` sizes="${sizes}"` : ''}>`;
});
engine.registerFilter('money', (c) => 'R$ ' + (Number(c) / 100).toLocaleString('pt-BR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }));
engine.registerFilter('json', (v) => JSON.stringify(v && v.__img ? v.src : v));
engine.registerFilter('handleize', (s) => String(s).toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, ''));
engine.registerFilter('placeholder_svg_tag', (n, cls) => `<svg class="${cls || ''}" viewBox="0 0 100 100"><rect width="100" height="100" fill="#ddd"/></svg>`);
engine.registerFilter('t', (s) => s);
// tags de bloco da Shopify
const block = (name, open, close) => engine.registerTag(name, {
  parse(tag, remain) { this.tpls = []; const stream = this.liquid.parser.parseStream(remain); stream.on('template', (t) => this.tpls.push(t)).on('tag:end' + name, () => stream.stop()).on('end', () => { throw new Error(`falta end${name}`); }); stream.start(); this.args = tag.args; },
  * render(ctx, emitter) { emitter.write(typeof open === 'function' ? open(this.args) : open); yield this.liquid.renderer.renderTemplates(this.tpls, ctx, emitter); emitter.write(close); },
});
block('stylesheet', '<style>', '</style>');
block('javascript', '<script>', '</script>');
block('form', (a) => { const id = (a.match(/id:\s*'([^']+)'/) || [])[1] || ''; const cls = (a.match(/class:\s*'([^']+)'/) || [])[1] || ''; return `<form method="post" action="/cart/add" id="${id}" class="${cls}" enctype="multipart/form-data">`; }, '</form>');
engine.registerTag('schema', { parse(tag, remain) { const s = this.liquid.parser.parseStream(remain); s.on('tag:endschema', () => s.stop()).on('template', () => {}).on('end', () => { throw new Error('falta endschema'); }); s.start(); }, render() { return ''; } });

const body = await engine.parseAndRender(src, { product, section, shop: { name: 'Engine Print' } });
const html = `<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${product.title} — mock</title>
<style>html{background:#f2f2f3;color:#0a0a0a;font-family:"Assistant",system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;font-size:16px}body{margin:0}h1,h2,h3{font-family:"Archivo",system-ui,sans-serif}.color-scheme-1{--page-width:1300px}.mock-header{height:64px;background:#fff;border-bottom:1px solid #e4e4e7;display:flex;align-items:center;padding:0 1rem;font-weight:700;letter-spacing:.06em}.mock-footer{height:600px;background:#0a0a0a;color:#fff;padding:2rem;margin-top:3rem}</style>
</head><body><div class="mock-header">ENGINE PRINT · mock</div>${body}<div class="mock-footer">rodapé (só para ter rolagem)</div></body></html>`;
writeFileSync(outPath, html);
console.log('ok', outPath, html.length, 'bytes; variantes:', variants.map((v) => `${v.title}=${v.available ? 'ok' : 'off'}/${v.inventory_quantity}`).join(' '));
