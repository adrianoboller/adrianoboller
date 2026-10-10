/* A CSP da pagina, provada no navegador (pedido 771).
 *
 * As tres provas de aceite do dono sao «segredo nunca no cliente, permissoes
 * checadas no servidor e CSP». Este caso cuida da primeira e da terceira pelo
 * EFEITO, e nao pela leitura do cabecalho:
 *
 *  1. O cabecalho: `script-src` sem `'unsafe-inline'` nem `'unsafe-eval'`,
 *     um `'sha256-…'` por bloco embutido, `script-src-attr 'none'`, e a
 *     moldura (`frame-ancestors`, `base-uri`, `object-src`, `form-action`,
 *     COOP, CORP).
 *  2. A pagina RODA sob ela: entra, abre as telas mais pesadas, e nenhuma
 *     violacao de script aparece -- a bateria inteira tambem vigia isso, em
 *     toda tela de todo caso (`VIGIA_DA_CSP` no `bateria.mjs`).
 *  3. O VENENO nao roda: um `<img src=x onerror=…>` posto por `innerHTML`,
 *     que e exatamente o que um dado sem `esc()` faria. Com a CSP velha
 *     (`script-src 'unsafe-inline'`) ele dispara -- e esta e a prova que cai
 *     com o defeito reposto. O veneno e um manipulador, e nao um `<script>`:
 *     `<script>` posto por `innerHTML` nunca roda, com ou sem CSP, e provar
 *     com ele daria verde pelo motivo errado (a mesma licao do 347).
 *  4. O `E()` do `claude.js` escapa de verdade: ate o 771 ele testava
 *     `window.esc`, que nao existe (o `esc` e um `const` de topo), e devolvia
 *     o texto CRU. Um endereco com `<b id=…>` virava elemento.
 *  5. Nada que de poder sozinho fica no armazenamento do navegador: nem a
 *     senha, nem a senha de execucao, nem o id da sessao, nem a chave da
 *     Claude que acabou de ser colada.
 *  6. O ESTILO tambem (pedido 771, segunda etapa): `style-src` sem
 *     `'unsafe-inline'`, um hash por `<style>` embutido, `style-src-attr
 *     'none'`. O veneno e um `<div style=…>` posto por `innerHTML` -- HTML
 *     injetado SEM script nenhum, que desenha um aviso falso por cima da
 *     tela. Com a CSP velha ele pinta; com a nova o navegador o barra e
 *     relata. E o estilo DINAMICO da casa continua de pe: as barras do Painel
 *     tem a largura pedida (o `PhxEstilo` a pos pelo CSSOM), e um `data-e-*`
 *     com valor fora do crivo e recusado em vez de virar estilo. */
import { entrar, abrirPeloMenu, assentar, verdade, igual, SENHA_EXECUCAO, CREDENCIAL } from '../apoio.mjs';

const CHAVE_FALSA = 'sk-ant-api03-' + 'prova771'.repeat(6);

/* As telas que mais montam HTML: o painel inicial ja abre no `entrar`. */
const TELAS = [
  'tela.mi_claude',
  'tela.mi_telemetria',
  'tela.mi_gerais_servidor',
  'tela.mi_idiomas',
];

export const caso = {
  nome: 'csp',
  async rodar(ctx) {
    const { page } = ctx;

    // ------------------------------------------------------- 1. cabecalho
    const r = await page.request.get(ctx.url);
    const h = r.headers();
    const csp = h['content-security-policy'] || '';
    const diretiva = nome => csp.split(';').map(d => d.trim())
      .find(d => d.split(' ')[0] === nome) || '';
    const script = diretiva('script-src');
    verdade(script, `a pagina nao declara script-src: ${csp}`);
    verdade(!script.includes("'unsafe-inline'"),
      `script-src ainda aceita script embutido sem hash: ${script}`);
    verdade(!script.includes("'unsafe-eval'"), `script-src aceita eval: ${script}`);
    igual(diretiva('script-src-attr'), "script-src-attr 'none'", 'manipulador em atributo barrado');
    for (const d of ["frame-ancestors 'none'", "base-uri 'none'", "object-src 'none'",
      "form-action 'none'", "default-src 'none'"]) {
      verdade(csp.includes(d), `falta «${d}» na CSP: ${csp}`);
    }
    const estilo = diretiva('style-src');
    verdade(estilo, `a pagina nao declara style-src: ${csp}`);
    verdade(!estilo.includes("'unsafe-inline'"),
      `style-src ainda aceita estilo embutido sem hash: ${estilo}`);
    igual(diretiva('style-src-attr'), "style-src-attr 'none'", 'atributo style barrado');
    igual(h['cross-origin-opener-policy'], 'same-origin', 'COOP');
    igual(h['cross-origin-resource-policy'], 'same-origin', 'CORP');
    igual(h['x-frame-options'], 'DENY', 'X-Frame-Options');
    igual(h['x-content-type-options'], 'nosniff', 'nosniff');

    // --------------------------------------- 2. a pagina roda sob a CSP
    // O ouvinte do PROPRIO caso, para ele poder contar a sonda; o da
    // bateria continua vigiando o resto.
    await page.addInitScript(() => {
      window.__viol771 = [];
      document.addEventListener('securitypolicyviolation', e => {
        window.__viol771.push({ d: e.effectiveDirective, s: !!window.__phx771Sonda });
      }, true);
    });
    await entrar(page, ctx.url);
    // Os blocos contados pelo NAVEGADOR, e nao por um `grep '<script>'` no
    // HTML: o script do `index.html` traz «<script>» dentro de um comentario
    // de JS, e quem conta texto acha oito onde o analisador de HTML ve sete.
    const blocos = await page.evaluate(() => document.querySelectorAll('script:not([src])').length);
    igual((script.match(/'sha256-/g) || []).length, blocos,
      `um hash por <script> embutido (${blocos} blocos)`);
    const folhas = await page.evaluate(() => document.querySelectorAll('style').length);
    igual((estilo.match(/'sha256-/g) || []).length, folhas,
      `um hash por <style> embutido (${folhas} blocos)`);

    // O estilo dinamico da casa, no Painel que abre com o `entrar`: as barras
    // sao `<i data-e-larg>`, e sem o `PhxEstilo` teriam largura zero, caladas.
    await assentar(page, 600);
    const barras = await page.evaluate(() => [...document.querySelectorAll('[data-e-larg]')]
      .map(e => ({ pedida: Number(e.getAttribute('data-e-larg')), tem: e.style.width })));
    verdade(barras.length > 0,
      'o Painel nao desenhou nenhuma barra com data-e-larg — a prova nao provaria nada');
    const semLargura = barras.filter(b => b.tem !== Math.max(0, Math.min(100, b.pedida)) + '%');
    igual(semLargura.length, 0, `barras sem a largura pedida: ${JSON.stringify(semLargura.slice(0, 3))}`);

    for (const t of TELAS) {
      await abrirPeloMenu(page, t);
      await assentar(page, 400);
    }
    const antes = await page.evaluate(() => window.__viol771.filter(v => !v.s));
    igual(antes.length, 0, `violacoes da CSP com a pagina em uso: ${JSON.stringify(antes)}`);

    // ------------------------------------------- 3. o veneno nao roda
    const veneno = await page.evaluate(async () => {
      window.__phx771Sonda = true;
      window.__xss771 = 0;
      const alvo = document.createElement('div');
      alvo.innerHTML = '<img src="x" onerror="window.__xss771=1">';
      document.body.appendChild(alvo);
      await new Promise(ok => setTimeout(ok, 600));
      alvo.remove();
      window.__phx771Sonda = false;
      return { disparou: window.__xss771,
               barrou: window.__viol771.filter(v => v.s && v.d === 'script-src-attr').length };
    });
    igual(veneno.disparou, 0, 'o onerror posto por innerHTML RODOU: a CSP nao barra manipulador');
    verdade(veneno.barrou >= 1,
      'o veneno nao rodou mas o navegador nao relatou violacao — a sonda nao provou nada');

    // ---------------------------- 6. o veneno de ESTILO nao pinta
    const pintura = await page.evaluate(async () => {
      window.__phx771Sonda = true;
      const conta = () => window.__viol771.filter(v => v.s && v.d === 'style-src-attr').length;
      const antes = conta();
      const alvo = document.createElement('div');
      alvo.innerHTML = '<div id="vene771" style="position:fixed;inset:0;z-index:99999;'
        + 'background:rgb(255,0,0)">aviso falso</div>';
      document.body.appendChild(alvo);
      await new Promise(ok => setTimeout(ok, 300));
      const cs = getComputedStyle(document.getElementById('vene771'));
      const pintou = cs.position === 'fixed' || cs.backgroundColor === 'rgb(255, 0, 0)';
      // O crivo do `PhxEstilo`: um valor que nao e cor nao vira estilo.
      const r0 = PhxEstilo.recusados;
      alvo.innerHTML = '<i id="crivo771" data-e-fundo="url(//exemplo.invalid/x)"></i>';
      await new Promise(ok => setTimeout(ok, 50));
      const crivo = { recusou: PhxEstilo.recusados - r0,
                      fundo: document.getElementById('crivo771').style.backgroundColor };
      alvo.remove();
      window.__phx771Sonda = false;
      return { pintou, crivo, barrou: conta() - antes };
    });
    verdade(!pintura.pintou, 'o style= posto por innerHTML PINTOU: a CSP nao barra atributo de estilo');
    verdade(pintura.barrou >= 1,
      'o estilo nao pintou mas o navegador nao relatou violacao — a sonda nao provou nada');
    igual(pintura.crivo.recusou, 1, 'o PhxEstilo aceitou um valor fora do crivo');
    igual(pintura.crivo.fundo, '', 'o PhxEstilo pintou um valor fora do crivo');

    // --------------------------------- 4. o E() do claude.js escapa
    const injecao = await page.evaluate(async () => {
      PhxIA._gravar({ endpoint: 'https://exemplo.invalid/<b id="inj771">x</b>' });
      PhxIA.telaConfig();
      await new Promise(ok => setTimeout(ok, 200));
      const virouElemento = !!document.getElementById('inj771');
      const texto = document.getElementById('painel').textContent.includes('<b id="inj771">');
      PhxIA._gravar({ endpoint: '' });
      return { virouElemento, texto };
    });
    verdade(!injecao.virouElemento,
      'o endereco da API virou HTML na tela da Claude: o E() devolveu o texto cru');
    verdade(injecao.texto, 'o endereco nao apareceu como TEXTO na tela da Claude');

    // ------------------------------ 5. nada que de poder no navegador
    const guardado = await page.evaluate(async chave => {
      PhxIA._gravar({ chave });
      PhxIA.telaConfig();
      await new Promise(ok => setTimeout(ok, 200));
      const tudo = [];
      for (const arm of [localStorage, sessionStorage]) {
        for (let i = 0; i < arm.length; i++) {
          const k = arm.key(i);
          tudo.push(k + '=' + arm.getItem(k));
        }
      }
      const sessao = est.sessao || '';
      PhxIA._gravar({ chave: '' });
      return { tudo, sessao };
    }, CHAVE_FALSA);
    verdade(guardado.sessao.length >= 16, 'a pagina nao tem sessao — a prova nao provaria nada');
    const proibidos = [
      ['a senha de login', CREDENCIAL.SENHA],
      ['a senha de execucao', SENHA_EXECUCAO],
      ['o id da sessao', guardado.sessao],
      ['a chave da Claude', CHAVE_FALSA],
    ];
    for (const [oQue, valor] of proibidos) {
      const onde = guardado.tudo.filter(x => x.includes(valor));
      verdade(!onde.length, `${oQue} ficou no armazenamento do navegador: ${onde.map(x => x.split('=')[0]).join(', ')}`);
    }
    ctx.notas.push(`${blocos} scripts e ${folhas} folhas por hash; ${barras.length} barra(s) pelo CSSOM; `
      + `${guardado.tudo.length} chave(s) no armazenamento, nenhuma com poder`);
  },
};
