/* A tela de entrada aparece mesmo quando a rede ENGOLE a fonte da marca.
 *
 * O caso reproduz a rede em que um servidor de banco realmente mora: um
 * firewall que DESCARTA o pacote em vez de recusar. O pedido a
 * `fonts.googleapis.com` fica pendurado, e uma folha de estilo bloqueante
 * segura o parser -- e com ele o primeiro `<script>` e o DOMContentLoaded.
 *
 * Medido antes do conserto, tres rodadas: 12,7 s de tela BRANCA. Depois:
 * 0,11 s. O piso de 3 s aqui e folgado de proposito -- ele nao existe para
 * medir a maquina, e sim para falhar redondo no dia em que a folha voltar a
 * bloquear.
 *
 * Desde 24/09/2026 a fonte vem EMBUTIDA (`phxsql_core::fontes`, em `data:`):
 * a pagina nao pede fonte a ninguem. O caso mudou de pergunta junto -- antes
 * provava que a fonte continuava sendo pedida (um conserto que tirasse a
 * fonte passaria no tempo e reprovaria a marca); hoje prova que ela NAO e
 * pedida fora e que, mesmo assim, a Exo 2 esta carregada. O buraco negro no
 * Google continua armado: se alguem voltar a pedir a fonte la, a tela volta
 * a esperar, e o caso acusa pelos dois lados. */
import { verdade } from '../apoio.mjs';

const TETO_MS = 3000;

export const caso = {
  nome: 'primeira-pintura',
  async rodar(ctx) {
    const { page } = ctx;

    let pediuAFonte = false;
    // O buraco negro: aceita o pedido e nunca responde. `abort()` seria uma
    // recusa imediata, que e justamente o caso FACIL -- e o que a versao
    // bloqueante tambem atravessava depressa.
    await page.route('**fonts.googleapis.com/**', async rota => {
      pediuAFonte = true;
      await new Promise(r => setTimeout(r, 30000));
      await rota.abort().catch(() => {});
    });
    await page.route('**fonts.gstatic.com/**', async rota => {
      await new Promise(r => setTimeout(r, 30000));
      await rota.abort().catch(() => {});
    });

    const t0 = Date.now();
    await page.goto(ctx.url, { waitUntil: 'commit' });
    await page.waitForSelector('#btEntrar', { state: 'visible', timeout: TETO_MS + 12000 });
    const ate = Date.now() - t0;

    ctx.notas.push(`tela de entrada visivel em ${ate} ms com a fonte pendurada`);
    // Sem captura aqui de proposito: o `screenshot` do Playwright espera as
    // fontes carregarem, e neste caso elas nunca carregam -- a captura
    // esperaria os 30 s do buraco negro e reprovaria o caso por um motivo que
    // nao e o dele.

    verdade(ate < TETO_MS,
      `a tela de entrada levou ${ate} ms para aparecer com o pedido da fonte pendurado — `
      + 'a folha da fonte voltou a bloquear a pintura');

    verdade(!pediuAFonte,
      'a pagina voltou a pedir a fonte ao Google Fonts: ela vem embutida no binario, '
      + 'e sem internet a marca sumiria');

    // A marca carregada SEM rede: o @font-face em data: resolveu a Exo 2.
    const exo = await page.evaluate(async () => {
      await document.fonts.load("600 16px 'Exo 2'");
      return document.fonts.check("600 16px 'Exo 2'");
    });
    verdade(exo, 'a Exo 2 embutida nao carregou: a tela saiu na fonte de reserva');

    // E a pilha de reserva continua la, para o dia em que a embutida falhar.
    const pilha = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
    verdade(/Exo 2/.test(pilha) && /Arial|sans-serif/.test(pilha),
      `a pilha de fontes perdeu a reserva: ${pilha}`);
  },
};
