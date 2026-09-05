/* A barra de ferramentas em resolucao menor: quebra em duas linhas ou rola?
 *
 *   node testes-web/barra-em-resolucao-menor.mjs
 *
 * Pergunta do dono, e ela NAO se responde lendo o CSS. O `#ferramentas`
 * declara `flex-wrap:wrap` no topo e `nowrap` numa media query de 1024px --
 * ler as duas regras diz o que foi ESCRITO, e nao o que a tela FAZ. O CSS
 * global desta casa ja mordeu componente novo duas vezes (o radio que virou
 * bolinha, o «Blumenau» que virou «BLUMENAU»), e as duas so apareceram
 * abrindo o navegador.
 *
 * O que esta sonda mede em cada largura, com o navegador de verdade:
 *
 *   fileiras   -- quantos `offsetTop` DISTINTOS os botoes ocupam. Duas
 *                 fileiras sao dois valores, e nao um palpite sobre altura.
 *   rola       -- `scrollWidth > clientWidth` na propria barra: quando ela
 *                 nao envolve, o resto tem de ficar alcancavel rolando.
 *   escondido  -- botao cujo direito passa do direito da barra E a barra nao
 *                 rola. E o unico estado REPROVADO: ferramenta inalcancavel.
 *   pagina     -- `scrollWidth > clientWidth` no documento. Barra larga nunca
 *                 pode empurrar a PAGINA para o lado; o desenho manda a
 *                 rolagem morrer dentro do componente.
 *
 * A captura de cada largura fica ao lado do JSON, porque numero de fileiras
 * nao mostra se a barra ficou feia -- so se ela ficou errada.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { subir, USUARIO, SENHA, TOKEN } from './servidor.mjs';

const PORTA_DADOS = 6320;
const PORTA_WEB = 6321;
const SAIDA = process.env.SAIDA
  || '/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/barra-responsiva';

/* As larguras que interessam, e o motivo de cada uma. O 1025 e o 1024 estao
 * ali de proposito: sao os dois lados do ponto de virada declarado na folha,
 * e e exatamente onde uma regra que nao pega apareceria. */
const LARGURAS = [
  [1920, 'monitor grande'],
  [1440, 'notebook'],
  [1280, 'notebook menor'],
  [1025, 'um pixel ACIMA da virada'],
  [1024, 'a virada declarada na folha'],
  [900, 'tablet deitado'],
  [768, 'tablet em pe'],
  [640, 'a segunda virada'],
  [414, 'celular grande'],
  [360, 'celular pequeno'],
];

async function medir(page, largura) {
  await page.setViewportSize({ width: largura, height: 900 });
  // Duas passadas de layout: mudar o viewport nao termina o reflow, e medir
  // dentro dele devolve a geometria da largura ANTERIOR.
  await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
  await page.waitForTimeout(250);
  return page.evaluate(() => {
    const barra = document.getElementById('ferramentas');
    if (!barra) return { erro: 'nao ha #ferramentas nesta tela' };
    const bs = getComputedStyle(barra);
    const botoes = [...barra.querySelectorAll('.fer')];
    const topos = [...new Set(botoes.map(b => Math.round(b.offsetTop)))].sort((a, b) => a - b);
    const rola = barra.scrollWidth > barra.clientWidth + 1;
    const dir = barra.getBoundingClientRect().right;
    // «Escondido» so e defeito quando a barra NAO rola: se ela rola, o botao
    // esta fora da vista mas alcancavel, que e o desenho.
    const passam = botoes.filter(b => b.getBoundingClientRect().right > dir + 1).length;
    return {
      fileiras: topos.length,
      topos,
      botoes: botoes.length,
      flexWrap: bs.flexWrap,
      alturaBarra: Math.round(barra.getBoundingClientRect().height),
      rola,
      sobraDeRolagem: Math.max(0, barra.scrollWidth - barra.clientWidth),
      escondidos: rola ? 0 : passam,
      paginaRolaDeLado: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
    };
  });
}

const phxsqld = join(process.cwd(), 'target/release/phxsqld');
mkdirSync(SAIDA, { recursive: true });

const srv = await subir({ phxsqld, portaDados: PORTA_DADOS, portaWeb: PORTA_WEB });
const url = `http://127.0.0.1:${PORTA_WEB}/`;
const navegador = await chromium.launch();
const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
const page = await ctx.newPage();

const linhas = [];
try {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 });
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo', { timeout: 20000 });
  await page.waitForSelector('#ferramentas .fer', { timeout: 20000 });

  for (const [largura, porque] of LARGURAS) {
    const m = await medir(page, largura);
    linhas.push({ largura, porque, ...m });
    await page.screenshot({ path: join(SAIDA, `barra-${String(largura).padStart(4, '0')}.png`),
                            clip: { x: 0, y: 0, width: largura, height: 260 } });
    const alerta = m.escondidos ? `  <<< ${m.escondidos} BOTAO(ES) INALCANCAVEL(EIS)`
      : (m.paginaRolaDeLado ? '  <<< A PAGINA ROLA DE LADO' : '');
    console.log(
      `${String(largura).padStart(4)}px  ${String(m.fileiras).padStart(2)} fileira(s)`
      + `  altura ${String(m.alturaBarra).padStart(3)}px`
      + `  wrap=${(m.flexWrap || '?').padEnd(6)}`
      + `  ${m.rola ? `rola (+${m.sobraDeRolagem}px)` : 'nao rola'.padEnd(14)}`
      + `  ${porque}${alerta}`);
  }
} finally {
  await navegador.close();
  await srv.derrubar();
}

writeFileSync(join(SAIDA, 'resultados.json'),
  JSON.stringify({ quando: new Date().toISOString(), botoes: linhas[0]?.botoes ?? 0, larguras: linhas }, null, 2));

const ruins = linhas.filter(l => l.escondidos > 0 || l.paginaRolaDeLado);
console.log(`\n${linhas.length} larguras medidas, captura de cada uma em ${SAIDA}`);
console.log(ruins.length ? `REPROVA em ${ruins.length}: ${ruins.map(r => r.largura + 'px').join(', ')}`
                         : 'Nenhuma ferramenta inalcancavel e nenhuma pagina rolando de lado.');
process.exit(ruins.length ? 1 : 0);

/* MEDIDO EM 05/09/2026, e o que a sonda achou por olhar so os BOTOES:
 *
 * Entre ~1576px e ~1589px a barra tem UMA fileira de botoes e mede 66px em vez
 * de 56 -- dez pixels de faixa morta. A causa nao e botao nem barra de
 * rolagem: e um TRACO separador (`.risco`) que envolve sozinho para a segunda
 * fileira. Ele fica com altura 0 (`risco(1x0)`), mas a linha de flex que ele
 * abre continua ocupando espaco.
 *
 * A primeira versao desta sonda contava so `#ferramentas .fer` e por isso dizia
 * «1 fileira» com dez pixels sobrando -- numero certo para a pergunta errada.
 * Quem for medir geometria de flex conte TODOS os filhos, e nao so os que
 * interessam: o que sobra na conta e justamente o que ninguem desenhou.
 */
