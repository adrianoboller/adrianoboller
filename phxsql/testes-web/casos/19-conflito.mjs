/* A janela de conflito de escrita, EXERCITADA -- e nao lida.
 *
 * A regra pétrea (`CLAUDE.md`): «Merge de conflito marca quem MEXEU, não quem
 * perguntou por último.» Ela mora em `dialogoConflito()`, dentro do
 * `include_str!` de 11 mil linhas que é a página -- não há `cargo test` que a
 * alcance. Antes deste caso, ela não tinha teste nenhum: nem unidade (é JS de
 * tela) nem ponta a ponta.
 *
 * O que este caso prova é a diferença entre as duas regras possíveis:
 *
 *   - a CERTA: cada coluna fica com quem a alterou -- a minha, comigo; a que
 *     só o outro mudou, com ele;
 *   - a ERRADA que a pétrea proíbe: tudo marcado "meu", que desfaria em
 *     silêncio o trabalho do outro nas colunas que eu nem toquei.
 *
 * Para isso, DUAS ABAS do mesmo navegador editam a MESMA linha -- a aba B
 * grava primeiro e muda só o UF; a aba A tinha aberto a ficha ANTES disso,
 * edita só a CIDADE (uma coluna diferente) e tenta gravar por cima. É
 * exatamente o "abriu às 9h02, voltou às 9h11" do comentário que introduz
 * `dialogoConflito()` no `index.html`. */
import {
  entrar, api, capturar, cenario, verdade, igual, contem, bancoDoCaso, abrirLinhaDaGrade,
} from '../apoio.mjs';

export const caso = {
  nome: 'conflito',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Conflito');
    const { tab } = await cenario(page, db);

    // A linha id=1 do cenário padrão: "Adriano Boller", Blumenau/SC.
    const lista = await api(page, 'varrer', { database: db, tabela: tab, max: 50 });
    const alvo = lista.linhas.find(l => l.id === 1);
    verdade(!!alvo, 'a linha id=1 do cenário padrão não apareceu no varrer');
    const rowid = alvo.rowid;

    // --------------------------------------------- ABA A abre a ficha (le a v1)
    await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await abrirLinhaDaGrade(page, { rowid });
    await page.waitForSelector('#fichaEdit');
    igual(await page.inputValue('#f_uf'), 'SC', 'a ficha (A) não abriu com o UF original');
    igual(await page.inputValue('#f_cidade'), 'Blumenau', 'a ficha (A) não abriu com a cidade original');

    // ------------------------------------- ABA B grava primeiro -- só o UF muda
    // Uma segunda aba, e não uma chamada de API crua: o que precisa ficar
    // provado é o par de sessões de verdade, cada uma passando pelo MESMO
    // caminho que uma pessoa usaria -- login, ficha, botão Salvar.
    const paginaB = await page.context().newPage();
    await entrar(paginaB, ctx.url);
    await paginaB.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
    await abrirLinhaDaGrade(paginaB, { rowid });
    await paginaB.waitForSelector('#fichaEdit');
    await paginaB.fill('#f_uf', 'PR');
    await paginaB.click('#btSalvar');
    await paginaB.waitForSelector('#btNova', { timeout: 10000 });
    await paginaB.close();

    // --------------------------- ABA A edita uma coluna DIFERENTE (cidade) e grava
    await page.fill('#f_cidade', 'Pomerode');
    await page.click('#btSalvar');

    // -------------------------------------------- a janela de conflito abre
    await page.waitForSelector('.caixa.larga[aria-label="Conflito de escrita"]', { timeout: 10000 });
    await capturar(ctx, ctx.nomeCaptura('conflito-aberto'));

    const linhasDaBriga = await page.locator('table.conf tbody tr.diverge').count();
    igual(linhasDaBriga, 2,
      'o diálogo não isolou exatamente as duas colunas que brigam (uf e cidade)');

    // O NÚCLEO da regra: marca quem MEXEU, não quem perguntou por último.
    // Se o diálogo marcasse tudo como "meu" (o defeito que a pétrea proíbe),
    // o UF -- que a aba A não tocou -- sairia marcado "meu" também, e gravar
    // apagaria a alteração da aba B sem ninguém ter escolhido isso.
    verdade(await page.isChecked('input[name="cf_uf"][value="outro"]'),
      'a coluna uf (que só a aba B mudou) não veio marcada com o valor do outro');
    verdade(!(await page.isChecked('input[name="cf_uf"][value="meu"]')),
      'a coluna uf veio marcada com "meu" -- e a aba A não tocou nela: é o defeito que a pétrea proíbe');
    verdade(await page.isChecked('input[name="cf_cidade"][value="meu"]'),
      'a coluna cidade (que a aba A digitou) não veio marcada com o meu valor');
    verdade(!(await page.isChecked('input[name="cf_cidade"][value="outro"]')),
      'a coluna cidade veio marcada com o valor do OUTRO -- e foi a aba A quem digitou');

    contem(await page.textContent('.caixa.larga'), 'outras 5 coluna',
      'o diálogo não contou as 5 colunas que não brigam (id, nome, limite, cadastro, ficha)');

    // ---------------------------------------------------- grava o escolhido
    await page.click('#btCfSim');
    await page.waitForSelector(
      '.caixa.larga[aria-label="Conflito de escrita"]', { state: 'detached', timeout: 10000 });

    // A PROVA final: as DUAS alterações sobrevivem -- nenhuma apagou a outra.
    const final = await api(page, 'ler', { database: db, tabela: tab, rowid });
    const linhaFinal = final.linha || final;
    igual(linhaFinal.uf, 'PR', 'o merge perdeu a alteração da aba B na coluna uf');
    igual(linhaFinal.cidade, 'Pomerode', 'o merge perdeu a alteração da aba A na coluna cidade');
  },
};
