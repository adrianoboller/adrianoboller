// Os cartoes do video (abertura, console, responsivo, fechamento). Todo
// numero sai de um arquivo medido -- nada aqui e digitado.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import fs from 'fs';
const D = process.env.DEMO || '/var/tmp/phx-demo';
const RAIZ = new URL('../../..', import.meta.url).pathname;
const ler = (f) => fs.readFileSync(f, 'utf8');
const bancada = ler(`${D}/bancada.txt`), autoteste = ler(`${D}/autoteste.txt`);
const tira = (re, t) => (t.match(re) || [, '?'])[1];
const cifra = tira(/Cifra[^|]*\|\s*([\d.]+) Mbit/, bancada);
const aperto = tira(/Aperto[^|]*\|\s*([\d.]+) ms/, bancada).replace('.', ',');
const x25519 = parseFloat(tira(/X25519\s*\|\s*([\d.]+) us/, bancada));
const mac1 = parseFloat(tira(/mac1\s*\|\s*([\d.]+) us/, bancada));
const doc = ler(`${RAIZ}phxvpn/docs/PHXVPN.md`);
const vazao = tira(/Vazão TCP pelo túnel \| \*\*(\d+) Mbit\/s/, doc);
const ping = tira(/membro → membro \| [\d,]+ ms \/ ([\d,]+) ms/, doc);
const feitos = (doc.match(/^- \[x\]/gm) || []).length, faltam = (doc.match(/^- \[ \]/gm) || []).length;
const pct = Math.round(100 * feitos / (feitos + faltam));
const sqlz = tira(/aceita_compressao` \| \*\*[\d.]+\*\* \(([\d,]+)×/, ler(`${RAIZ}phxsql/docs/CIFRA-DO-FIO.md`));
const dados = JSON.parse(ler(`${D}/dados.json`));
const resp = JSON.parse(ler(`${D}/responsivo.json`));
const faltaLista = [...doc.matchAll(/^- \[ \] (.*)$/gm)].map((m) => m[1].replace(/\*\*/g, '').replace(/`/g, '').replace(/^Segurança \w+ \([^)]*\): /, '').split(' — ')[0].split(' (')[0]);

const css = `@font-face{font-family:"Exo 2";font-weight:500;src:url(exo2-500.ttf)}@font-face{font-family:"Exo 2";font-weight:700;src:url(exo2-700.ttf)}
*{box-sizing:border-box}body{margin:0;width:1280px;height:720px;background:radial-gradient(circle at 80% 10%,#16204a 0,#010418 55%);color:#DDE2EB;
font:18px/1.45 "Exo 2",system-ui,sans-serif;display:grid;place-items:center;overflow:hidden}
.q{width:1160px;animation:e .8s ease both}@keyframes e{from{opacity:0;transform:translateY(14px)}}
h1{font:700 64px/1 "Exo 2";margin:0}h1 b{color:#FF4D10}h2{font:700 34px/1.2 "Exo 2";margin:0 0 18px}.sub{color:#8a93a8;font-size:22px;margin:10px 0 28px}
.grade{display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:16px}.k{background:#0a1122;border:1px solid #1c2742;border-radius:14px;padding:16px 18px}
.k b{display:block;font:700 36px/1.1 "Exo 2";color:#fff}.k b i{font-style:normal;color:#FF4D10}.k span{color:#8a93a8;font-size:15px}
.barra{height:16px;border-radius:9px;background:#1c2742;overflow:hidden;margin:8px 0 6px}.barra div{height:100%;background:linear-gradient(90deg,#FF4D10,#FFC43D);width:${pct}%}
pre{margin:0;font:14px/1.35 ui-monospace,monospace;color:#3ecf8e;background:#050a1c;border:1px solid #1c2742;border-radius:12px;padding:14px}
.lado{display:grid;grid-template-columns:1fr 1fr;gap:18px}.pe{color:#8a93a8;font-size:15px;margin-top:18px}
.tel{display:grid;grid-template-columns:190px 1fr;gap:20px;align-items:start}.tel img{width:100%;border:1px solid #1c2742;border-radius:12px}
ul{margin:8px 0 0;padding-left:20px;color:#8a93a8;font-size:16px}`;
const pagina = (corpo) => `<!doctype html><meta charset=utf-8><style>${css}</style><div class=q>${corpo}</div>`;
const cartoes = [
  ['abertura', 8, pagina(`<h1><b>phx</b>vpn</h1><p class=sub>Redes privadas entre computadores — seguras, rápidas e sem complicação.</p>
   <div class=grade><div class=k><b>P2P</b><span>sem servidor no meio — ou servidor próprio com OpenVPN, à escolha</span></div>
   <div class=k><b>Ponta a ponta</b><span>cifra Noise (a família do WireGuard), conferida contra o vetor oficial</span></div>
   <div class=k><b>Um binário</b><span>Linux e Windows, sem bibliotecas de fora; instala por .deb, .msi ou serviço</span></div></div>
   <p class=pe>Status do plano: <b>${pct}%</b> entregue (${feitos} de ${feitos + faltam} itens)</p><div class=barra><div></div></div>`)],
  ['console', 8, pagina(`<h2>Console <b style="color:#FF4D10">phxvpncmd</b>: prova e medida na própria máquina</h2>
   <div class=lado><pre>${autoteste.replace(/</g, '&lt;')}</pre><pre>${bancada.replace(/</g, '&lt;')}</pre></div>
   <p class=pe>Saída real, gravada para este vídeo. Autoteste contra RFC 7677 e o vetor oficial do Noise; bancada em release, um núcleo.</p>`)],
  ['responsivo', 9, pagina(`<h2>A mesma interface, do celular ao ultrawide</h2>
   <div class=tel><img src="resp-390.png"><img src="resp-1920.png"></div>
   <p class=pe>CSS grid + flexbox + container queries. Medido: rolagem lateral zero em ${resp.map((r) => r.largura).join(', ')} px; colunas da grade: ${resp.map((r) => r.colunas).join(' → ')}.</p>`)],
  ['fechamento', 12, pagina(`<h2>Números medidos — nenhum digitado</h2><div class=grade>
   <div class=k><b>${vazao} <i>Mbit/s</i></b><span>TCP pelo túnel P2P</span></div>
   <div class=k><b>${cifra} <i>Mbit/s</i></b><span>cifra num só núcleo</span></div>
   <div class=k><b>${aperto} <i>ms</i></b><span>aperto de mão completo</span></div>
   <div class=k><b>${Math.round(x25519 / mac1)}<i>×</i></b><span>mais barato recusar ataque de inundação (mac1)</span></div>
   <div class=k><b>${ping} <i>ms</i></b><span>ping entre membros pelo OpenVPN real, TLS 1.3</span></div>
   <div class=k><b>${sqlz}<i>×</i></b><span>retorno SQL comprimido (PhxSql, 50.000 linhas)</span></div>
   <div class=k><b>${dados.testes_linux} + ${dados.testes_windows}</b><span>testes verdes: Linux + Windows</span></div>
   <div class=k><b>${pct}<i>%</i></b><span>do plano entregue (${feitos} de ${feitos + faltam})</span></div></div>
   <p class=pe>Falta: ${faltaLista.join(' · ')}.</p>`)],
];
for (const f of ['exo2-500.ttf', 'exo2-700.ttf']) fs.copyFileSync(`${process.env.FONTES}/${f}`, `${D}/${f}`);
const b = await chromium.launch();
for (const [nome, seg, html] of cartoes) {
  fs.writeFileSync(`${D}/cartao-${nome}.html`, html);
  const ctx = await b.newContext({ viewport: { width: 1280, height: 720 }, recordVideo: { dir: `${D}/video-cartoes`, size: { width: 1280, height: 720 } } });
  const p = await ctx.newPage(); await p.goto(`file://${D}/cartao-${nome}.html`); await p.waitForTimeout(seg * 1000);
  const v = await p.video().path(); await ctx.close(); fs.renameSync(v, `${D}/cartao-${nome}.webm`);
  console.log('cartao', nome, seg + 's');
}
await b.close();
console.log(JSON.stringify({ pct, feitos, faltam, cifra, aperto, vazao, ping, sqlz, inundacao: Math.round(x25519 / mac1) }));
