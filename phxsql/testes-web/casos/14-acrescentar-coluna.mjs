/* Acrescentar coluna PELA TELA, numa tabela que já tem dado.
 *
 * O que este caso trava, e por que ele não podia ser um `cargo test`: o
 * cartão do editor de modelo passou versões dizendo, em português cravado,
 * que alterar coluna «não existe no servidor». A operação passou a existir no
 * sprint 25 — e uma tela que continua dizendo que não dá é tão errada quanto
 * uma que oferece um formulário que não salva. As duas mentiras são
 * invisíveis para o motor, que está certo dos dois lados.
 *
 * Ele exercita o caminho inteiro: abrir a aba Estrutura, clicar no botão,
 * preencher, salvar, e conferir que a coluna aparece na Estrutura E no
 * Conteúdo — com o dado antigo intacto.
 *
 * E confere a armadilha do CSS global no componente novo, que é a lição do
 * «Blumenau» virando «BLUMENAU»: o cartão é um formulário dentro de um modal,
 * e o `input{width:100%}`/`label{text-transform:uppercase}` da folha global
 * morde todo componente novo. */
import { entrar, api, capturar, verdade, contem, bancoDoCaso, abrirPelaArvore } from '../apoio.mjs';

export const caso = {
  nome: 'acrescentar-coluna',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Alter');

    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: 'clientes',
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'nome', tipo: 'Str(40)', obrigatoria: true },
        { nome: 'cidade', tipo: 'Str(30)' },
      ],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }).catch(() => {});
    // «Blumenau» de propósito: é o dado com que a armadilha do
    // `text-transform:uppercase` se pega. Dado já em maiúscula não provaria nada.
    for (const l of [[1, 'Adriano Boller', 'Blumenau'], [2, 'Maria Souza', 'Joinville']]) {
      await api(page, 'inserir', { database: db, tabela: 'clientes', valores: l }).catch(() => {});
    }

    // --- 1. a aba Estrutura tem o botão, e ele abre o cartão
    await abrirPelaArvore(page, db, 'clientes');
    await page.evaluate(() => irAba('estrutura'));
    await page.waitForSelector('#estAddCol', { timeout: 15000 });
    await capturar(ctx, ctx.nomeCaptura('estrutura-com-botao'));

    await page.click('#estAddCol');
    await page.waitForSelector('#acNome', { timeout: 15000 });

    // O cartão diz o PREÇO antes de qualquer campo: quem clica precisa saber
    // que isto reescreve o arquivo inteiro.
    const cartao = await page.textContent('.er-cartao-corpo');
    contem(cartao, 'reescreve',
      'o cartão não diz que acrescentar coluna reescreve o arquivo de dados');
    contem(cartao, 'rowid',
      'o cartão não diz que o rowid não muda — que é a razão de os índices não serem refeitos');

    // --- 2. a armadilha do CSS global, no componente novo
    const largura = await page.$eval('#acObrig', el => el.getBoundingClientRect().width);
    verdade(largura > 0 && largura < 40,
      `a caixa de marcar do cartão ficou com ${Math.round(largura)}px — o `
      + '`input{width:100%}` da folha global a esticou');

    await capturar(ctx, ctx.nomeCaptura('cartao-vazio'));

    // --- 3. preencher e salvar
    await page.fill('#acNome', 'situacao');
    await page.selectOption('#acTipo', 'Str(60)');
    await page.fill('#acCaption', 'Situação');
    await page.fill('#acPadrao', 'ativo');
    await page.click('#acFazer');
    // O cartão fecha e a aba se redesenha.
    await page.waitForSelector('#acNome', { state: 'detached', timeout: 20000 });
    await page.waitForTimeout(800);

    // --- 4. a coluna existe, e o dado antigo continua lá
    const esquema = await api(page, 'esquema', { database: db, tabela: 'clientes' });
    const nomes = (esquema.colunas || []).map(c => c.nome);
    verdade(nomes.includes('situacao'),
      `a coluna nova não entrou no esquema: ${JSON.stringify(nomes)}`);
    verdade(nomes.indexOf('situacao') < nomes.indexOf('softdeleted'),
      `a coluna nova entrou DEPOIS das de sistema: ${JSON.stringify(nomes)}`);

    await page.evaluate(() => irAba('estrutura'));
    await page.waitForSelector('#estAddCol', { timeout: 15000 });
    await page.waitForTimeout(300);
    contem(await page.textContent('#painel'), 'situacao',
      'a aba Estrutura não mostra a coluna que acabou de nascer');
    await capturar(ctx, ctx.nomeCaptura('estrutura-depois'));

    // --- 5. o Conteúdo: o dado antigo intacto e a coluna nova preenchida
    await page.evaluate(() => irAba('conteudo'));
    await page.waitForTimeout(900);
    const conteudo = await page.textContent('#painel');
    contem(conteudo, 'Blumenau',
      'o dado antigo sumiu da grade depois de acrescentar a coluna');
    verdade(!/BLUMENAU/.test(conteudo),
      'a grade mostrou «BLUMENAU» — o `text-transform` da folha global está mentindo sobre o dado');
    contem(conteudo, 'ativo',
      'a coluna nova não aparece preenchida nas linhas que já existiam');
    await capturar(ctx, ctx.nomeCaptura('conteudo-depois'));

    // --- 6. a recusa também tem de aparecer NA TELA, e não só no log
    await page.evaluate(() => irAba('estrutura'));
    await page.waitForSelector('#estAddCol', { timeout: 15000 });
    await page.click('#estAddCol');
    await page.waitForSelector('#acNome', { timeout: 15000 });
    await page.fill('#acNome', 'cnpj');
    await page.check('#acObrig');
    await page.click('#acFazer');
    await page.waitForSelector('#acRecado .aviso', { timeout: 20000 });
    // A frase do motor vem sem acento (regra da casa); o que importa é que ela
    // CHEGA à tela, e com o motivo, em vez de o cartão fechar como se tivesse
    // dado certo.
    contem(await page.textContent('#acRecado'), 'inventar dado',
      'a recusa da coluna obrigatória sem padrão não apareceu no cartão');
    await capturar(ctx, ctx.nomeCaptura('cartao-recusa'));
    await page.click('#acFechar');
  },
};
