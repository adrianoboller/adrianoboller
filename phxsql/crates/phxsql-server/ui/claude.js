/* Integração com a Claude (API da Anthropic), no Centro de Controle.
 *
 * ## Por que a chamada sai do NAVEGADOR, e não do servidor
 *
 * A API da Anthropic é HTTPS obrigatório, e a `std` do Rust não tem TLS. Um
 * servidor que falasse com ela precisaria de uma crate de TLS — e a primeira
 * regra da casa é zero dependências externas. As três saídas eram: acrescentar
 * a crate, escrever TLS aqui dentro, ou não passar pelo servidor. A escolhida
 * foi a terceira, e ela é melhor do que um contorno: o servidor NUNCA vê a
 * chave, nunca faz a chamada e não precisa de TLS. A chave é de quem usa, mora
 * no `localStorage` do navegador dele, e cada pessoa paga a própria conta.
 *
 * O corolário é a regra que este arquivo inteiro respeita: **a chave não entra
 * em nenhum pedido ao PhxSql.** Ela só aparece no cabeçalho `x-api-key` do
 * `fetch` que vai para `api.anthropic.com` — e é o mesmo naipe da senha, que
 * nunca vai em texto puro para arquivo, log ou resposta do protocolo.
 *
 * ## Arquivo próprio, como o `diagrama-er.js`
 *
 * Pelo mesmo motivo: o `index.html` já tem dez mil linhas, e três agentes
 * mexem nele ao mesmo tempo. Lá ficam só duas coisas — o item do menu
 * Configurações e o botão da tela de Query. Todo o resto está aqui.
 *
 * ## O que viaja, e o que não viaja
 *
 * Viaja o ESQUEMA (nomes de tabela, de coluna, tipos, chaves, índices), porque
 * é ele que faz a resposta acertar. Ele sai do `esquema` do protocolo, que
 * passa pelo portão de permissão normal — tabela que este usuário não pode ler
 * simplesmente não entra no contexto.
 *
 * NÃO viaja LINHA de dado, a menos que a pessoa marque a caixa naquela
 * chamada. E mesmo marcada, coluna marcada como dado pessoal (a marcação de
 * LGPD que o próprio esquema carrega) sai REDIGIDA: o valor vira `"***"` por
 * ANÁLISE do objeto, coluna a coluna — não por recorte de texto.
 *
 * O painel «o que vai subir» mostra o corpo exato ANTES de enviar. Ninguém
 * deve descobrir depois que mandou o esquema do cliente para fora.
 */
"use strict";

window.PhxIA = (function () {

  /* ------------------------------------------------------------ contrato
     O endereço, a versão e o cabeçalho de acesso direto do navegador.

     `anthropic-dangerous-direct-browser-access: true` é o cabeçalho que a
     API exige de quem chama do navegador; sem ele a chamada volta com erro de
     CORS. O nome foi CONFERIDO no código do SDK oficial da Anthropic para
     TypeScript (`anthropics/anthropic-sdk-typescript`, `src/client.ts`), que o
     manda exatamente assim quando `dangerouslyAllowBrowser` está ligado — não
     foi escrito de memória.

     O endereço fica numa constante só, e o `endpoint` da configuração existe
     para poder apontar a tela para um servidor de mentira ao exercitar o
     caminho inteiro sem gastar chave. A tela DIZ quando ele não é o oficial,
     porque endereço trocado calado seria a forma mais fácil de desviar uma
     chave. */
  const ENDPOINT_OFICIAL = "https://api.anthropic.com/v1/messages";
  const VERSAO_API = "2023-06-01";
  const CABECALHO_NAVEGADOR = "anthropic-dangerous-direct-browser-access";

  /* Os três modelos oferecidos. A escolha é de CUSTO, e a tela diz isso: o
     padrão é o mais capaz, e quem quiser gastar menos desce a lista. */
  const MODELOS = [
    { id: "claude-opus-5",    rot: "Claude Opus 5",   diz: "o mais capaz — o padrão" },
    { id: "claude-sonnet-5",  rot: "Claude Sonnet 5", diz: "intermediário, custa menos" },
    { id: "claude-haiku-4-5", rot: "Claude Haiku 4.5", diz: "o mais barato e o mais rápido" },
  ];
  const MODELO_PADRAO = "claude-opus-5";

  /* 4000 chega para um SELECT e a explicação dele. Como a leitura é em
     streaming, um teto maior não trava a tela — mas também não há por que
     pagar por um que não se usa. */
  const MAX_TOKENS = 4000;

  /* ------------------------------------------------------- configuração
     Tudo no `localStorage`, que é do navegador de quem usa e nunca chega ao
     servidor. Em janela privada o acesso pode ESTOURAR, e não só voltar
     vazio — por isso todo toque é dentro de try/catch e a tela desenha certo
     sem valor guardado. */
  const GAVETA = "phxsql.ia";

  function cfg() {
    const padrao = { chave: "", modelo: MODELO_PADRAO, ligado: false,
                     endpoint: ENDPOINT_OFICIAL };
    try {
      const c = JSON.parse(localStorage.getItem(GAVETA) || "{}");
      return Object.assign(padrao, c && typeof c === "object" ? c : {});
    } catch { return padrao; }
  }

  function gravar(mudanca) {
    const c = Object.assign(cfg(), mudanca);
    try { localStorage.setItem(GAVETA, JSON.stringify(c)); } catch { /* privada */ }
    return c;
  }

  /** Ligada é ter chave E interruptor. Sem as duas, a tela de Query não muda
   *  em nada — que é o comportamento VELHO, e é o que o teste trava. */
  function ligada() {
    const c = cfg();
    return !!(c.ligado && c.chave);
  }

  /** Só os quatro últimos, como cartão de crédito. Nunca a chave inteira. */
  function fim(chave) {
    return chave ? "····" + chave.slice(-4) : "";
  }

  const oficial = c => (c.endpoint || ENDPOINT_OFICIAL) === ENDPOINT_OFICIAL;

  /* ------------------------------------------------------------- chamada */

  /** Monta o corpo do pedido. Sem prefill de assistente: os modelos atuais o
   *  recusam com 400, então o formato se controla pelo `system`. */
  function corpo(receita, pergunta, contexto, modelo) {
    const partes = [receita.sistema];
    if (contexto) partes.push("Contexto do banco de dados:\n" + contexto);
    return {
      model: modelo,
      max_tokens: MAX_TOKENS,
      stream: true,
      system: partes.join("\n\n"),
      messages: [{ role: "user", content: pergunta }],
    };
  }

  /** Os cabeçalhos. A chave sai daqui e de mais lugar nenhum. */
  function cabecalhos(chave) {
    return {
      "content-type": "application/json",
      "x-api-key": chave,
      "anthropic-version": VERSAO_API,
      [CABECALHO_NAVEGADOR]: "true",
    };
  }

  /** Traduz a recusa em recado que diz O QUE FAZER, e não só «erro».
   *
   *  A forma do erro da API é `{"type":"error","error":{"type":…,"message":…}}`
   *  — e ela é ANALISADA, não recortada: o que não vira JSON vira o código
   *  HTTP e o tamanho, porque não há como ler um campo de uma estrutura que
   *  não se lê. */
  function recado(codigo, texto) {
    let msg = "";
    try {
      const j = JSON.parse(texto);
      msg = (j && j.error && j.error.message) || "";
    } catch { msg = ""; }
    const detalhe = msg ? ` A API disse: «${msg}»` : ` (${texto.length} bytes de resposta)`;
    if (codigo === 401)
      return "A chave não foi aceita (401). Confira se ela está inteira e ainda "
           + "válida em Configurações → Integração com a Claude." + detalhe;
    if (codigo === 402)
      return "Cobrança pendente na conta da Anthropic (402). Acerte o pagamento "
           + "no Console dela e tente de novo." + detalhe;
    if (codigo === 403)
      return "Esta chave não tem permissão para este recurso (403)." + detalhe;
    if (codigo === 429)
      return "Limite de uso atingido (429). Espere um pouco e peça de novo — ou "
           + "escolha um modelo mais barato em Configurações." + detalhe;
    if (codigo === 400)
      return "A API recusou o pedido (400)." + detalhe;
    if (codigo === 529 || codigo >= 500)
      return `A API está sobrecarregada ou fora do ar (${codigo}). Não é a sua `
           + "chave nem o seu pedido: tente de novo em alguns segundos." + detalhe;
    return `A API respondeu ${codigo}.` + detalhe;
  }

  /** Faz a chamada e vai entregando os pedaços conforme chegam.
   *
   *  Streaming porque a alternativa é a tela parada esperando o texto inteiro.
   *  Os eventos que importam são três: `message_start` traz os tokens de
   *  entrada, `content_block_delta` traz cada pedaço em `delta.text`, e
   *  `message_delta` fecha os tokens de saída. `message_stop` encerra.
   *
   *  Erro no MEIO do fluxo chega como `event: error` com HTTP 200 — quem só
   *  olha o código de status não o vê. */
  async function perguntar(receita, pergunta, contexto, aoPedaco) {
    const c = cfg();
    if (!c.chave) throw new Error("Sem chave configurada.");
    const alvo = c.endpoint || ENDPOINT_OFICIAL;

    let r;
    try {
      r = await fetch(alvo, {
        method: "POST",
        headers: cabecalhos(c.chave),
        body: JSON.stringify(corpo(receita, pergunta, contexto, c.modelo || MODELO_PADRAO)),
      });
    } catch (e) {
      // Rede caída, DNS, ou a política de segurança da página barrando o
      // endereço. O `fetch` não distingue os três de propósito (contar isso
      // ao script seria contar demais a quem quer sondar), então o recado
      // cobre os três.
      throw new Error("Não deu para alcançar a API da Anthropic. Confira a "
        + "conexão desta máquina com a internet e o endereço configurado ("
        + alvo + "). Detalhe do navegador: " + (e && e.message ? e.message : e));
    }

    if (!r.ok) throw new Error(recado(r.status, await r.text()));

    const uso = { entrada: 0, saida: 0 };
    let inteiro = "";
    const leitor = r.body.getReader();
    const dec = new TextDecoder();
    let resto = "";
    for (;;) {
      const { done, value } = await leitor.read();
      if (done) break;
      resto += dec.decode(value, { stream: true });
      // Um evento SSE termina em linha em branco. Guardar o resto é o que
      // impede um pedaço partido ao meio de virar JSON inválido.
      let corte;
      while ((corte = resto.indexOf("\n\n")) >= 0) {
        const bloco = resto.slice(0, corte);
        resto = resto.slice(corte + 2);
        for (const linha of bloco.split("\n")) {
          if (!linha.startsWith("data:")) continue;
          let ev;
          try { ev = JSON.parse(linha.slice(5).trim()); } catch { continue; }
          if (ev.type === "content_block_delta" && ev.delta
              && typeof ev.delta.text === "string") {
            inteiro += ev.delta.text;
            if (aoPedaco) aoPedaco(inteiro, uso);
          } else if (ev.type === "message_start" && ev.message && ev.message.usage) {
            uso.entrada = ev.message.usage.input_tokens || 0;
            uso.saida = ev.message.usage.output_tokens || 0;
          } else if (ev.type === "message_delta" && ev.usage) {
            uso.saida = ev.usage.output_tokens || uso.saida;
          } else if (ev.type === "error") {
            const m = ev.error && ev.error.message ? ev.error.message : "erro no meio da resposta";
            throw new Error("A resposta foi interrompida pela API: " + m);
          } else if (ev.type === "message_stop") {
            resto = "";
          }
        }
      }
    }
    return { texto: inteiro, uso };
  }

  /* ------------------------------------------------------------ receitas
     O `system` de cada uma. Todas ensinam o vocabulário REAL do motor: uma
     proposta com tipo que não existe aqui é lixo, e um SQL com cláusula que a
     camada não tem é pior — parece que funciona. */

  /* O vocabulário REAL do motor, escrito na forma que o motor ACEITA.
     `Decimal(15,2)` e não `Decimal{15,2}`: o analisador de tipos do servidor
     (`valores.rs`) lê a forma com parênteses, e a chave só aparece quando o
     esquema é lido de volta, porque a leitura imprime o `Debug` do Rust.
     Ensinar a forma errada aqui produziria proposta que não cria tabela — que
     é exatamente o «tipo que não existe aqui». */
  const TIPOS = `Os tipos de coluna do PhxSql, e SÓ estes existem — escreva-os
exatamente assim, que é como o motor os aceita:
Bool; Int1 Int2 Int4 Int8; UInt1 UInt2 UInt4 UInt8; Real4 Real8;
Decimal(precisao,escala) — exato, até 38 dígitos; dinheiro é Decimal, nunca Real;
Date (dias desde 1970-01-01); Time (centésimos de segundo desde a meia-noite);
DateTime (milissegundos desde 1970-01-01T00:00:00Z);
Str(n) — texto UTF-8 de largura fixa, n até 65535 bytes;
Bin — binário de tamanho livre, mora no arquivo .bin;
Memo — texto longo, mora no arquivo .memo;
Uuid (128 bits em 16 bytes); Uuid256 (256 bits em 32 bytes, cabe um SHA-256);
Sequence — contador crescente da tabela, atribuído na inserção; só um por tabela.
Não existem VARCHAR, TEXT, INT, BIGINT, FLOAT, SERIAL, BOOLEAN nem JSON: use o
nome do PhxSql. Ao LER o esquema, o Decimal aparece como
"Decimal { precisao: 15, escala: 2 }" — é a mesma coisa impressa de outro jeito.
Índice mora num arquivo .ndx e pode ser único, composto, descendente e NOCASE.
Chave estrangeira tem ação ao excluir e ao alterar (restringir, cascata,
anular ou nada).`;

  const SQL_QUE_EXISTE = `A camada SQL do PhxSql é NOVA e entende só isto:
SELECT ( * | COUNT(*) | coluna [AS apelido] {, ...} )
FROM   [database.] [schema.] tabela [[AS] apelido]
[WHERE coluna ( = | <> | < | <= | > | >= ) literal]
[ORDER BY coluna [ASC|DESC]]
[LIMIT n [OFFSET m]]
Ainda NÃO existem: AND, OR, LIKE, IN, BETWEEN, IS NULL, DISTINCT, GROUP BY,
HAVING, JOIN, subconsulta, os agregados que não sejam COUNT(*), e
INSERT/UPDATE/DELETE. O WHERE e o ORDER BY só valem sobre coluna que TENHA
índice — sem índice o motor recusa, dizendo o nome da cláusula, em vez de
varrer a tabela calado. Sem ORDER BY a ordem é a de DIGITAÇÃO.`;

  const RECEITAS = {
    sql: {
      rot: "Texto → SQL",
      ico: "⌕",
      pede: "Descreva em português o que você quer consultar",
      exemplo: "os dez últimos clientes cadastrados",
      esquema: true,
      editor: true,
      sistema: `Você ajuda a escrever consultas para o PhxSql, um motor de dados
próprio. Responda em português do Brasil.

${SQL_QUE_EXISTE}

Responda SOMENTE com o comando SQL, em uma linha ou em linhas, sem cerca de
código, sem comentário e sem explicação. Se o que foi pedido não couber no
subconjunto acima, responda uma única linha começando por "-- não dá: " e
diga qual cláusula falta.`,
    },
    explicar: {
      rot: "Explicar o SQL",
      ico: "☰",
      pede: "Cole a consulta que você quer entender",
      exemplo: "SELECT nome FROM clientes WHERE id = 7",
      esquema: true,
      editor: false,
      sistema: `Você explica consultas do PhxSql para quem vai mantê-las.
Responda em português do Brasil, em prosa curta.

${SQL_QUE_EXISTE}

Diga, nesta ordem: o que a consulta devolve; de onde ela lê; o que o WHERE
filtra; em que ordem as linhas saem; e o que pode surpreender (sem ORDER BY a
ordem é a de digitação; COUNT(*) lê o cabeçalho e não as linhas). Não invente
cláusula que a consulta não tem.`,
    },
    desempenho: {
      rot: "Índice / desempenho",
      ico: "◷",
      pede: "Cole a consulta que está lenta",
      exemplo: "SELECT * FROM pedidos WHERE cliente_id = 42",
      esquema: true,
      editor: false,
      sistema: `Você sugere índices e reescritas para o PhxSql.
Responda em português do Brasil.

${SQL_QUE_EXISTE}
${TIPOS}

O que importa neste motor: o WHERE e o ORDER BY exigem índice na coluna; o
índice mora num arquivo .ndx separado; escrever custa mais com cada índice a
mais, porque cada um é uma descida de B+tree por linha inserida.

Termine SEMPRE com a linha:
"Isto é sugestão a MEDIR, não verdade — meça antes de aceitar."
Não afirme ganho em número: você não mediu nada.`,
    },
    modelar: {
      rot: "Modelar tabelas",
      ico: "⛁",
      pede: "Descreva o negócio a modelar",
      exemplo: "uma loja com clientes, pedidos e itens de pedido",
      // O esquema do banco vai junto: é ele que deixa a proposta CRESCER sobre
      // o que já existe em vez de propor de novo o que já está criado, e é
      // dele que sai a conferência de colisão de nome.
      esquema: true,
      editor: false,
      // Esta receita não devolve prosa: devolve um PLANO que a tela sabe
      // conferir e executar. O formato se pede pelo `system`, porque prefill
      // de assistente é recusado com 400 nos modelos atuais.
      plano: true,
      sistema: `Você propõe modelos de dados para o PhxSql, e só usa o
vocabulário dele.

${TIPOS}

Responda SOMENTE com um objeto JSON, sem cerca de código e sem texto fora dele,
exatamente nesta forma:

{"tabelas":[{"nome":"clientes",
  "porque":"para que serve esta tabela, em uma linha",
  "colunas":[{"nome":"id","tipo":"Sequence","obrigatoria":true,
              "caption":"Código","dado_pessoal":"nao"}],
  "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true,
              "porque":"por que este índice vale a escrita mais lenta"}],
  "relacionamentos":[{"nome":"fk_cliente","colunas":["cliente_id"],
              "tabela_ref":"clientes","colunas_ref":["id"],
              "ao_excluir":"restringir","ao_alterar":"restringir",
              "porque":"em uma linha"}]}],
 "notas":["o que você decidiu e por quê, em português"]}

Regras que o PhxSql impõe e que a proposta tem de respeitar:
- "dado_pessoal" é "nao", "pessoal" ou "sensivel" — marque nome, CPF, e-mail,
  telefone, endereço como "pessoal", e saúde/biometria como "sensivel";
- no máximo UMA coluna Sequence por tabela;
- toda tabela precisa de um índice primário (unico e primario verdadeiros);
- índice e relacionamento só podem citar colunas que a própria proposta declara;
- NÃO proponha alterar tabela que já existe no contexto: o PhxSql não tem
  ALTER de coluna. Se algo precisa mudar numa tabela existente, diga isso em
  "notas" e proponha uma tabela NOVA em vez de alterar a antiga;
- as ações de "ao_excluir"/"ao_alterar" são restringir, cascata, anular ou nada.`,
    },
  };

  /* ============================================ o plano, e o que o motor aceita

     A criação de verdade reusa as operações que já existem e já são provadas:
     `criar_tabela` para a tabela com colunas e índices, e `declarar_fk` para
     cada relacionamento. NÃO há um segundo caminho de criação aqui -- dois
     caminhos divergiriam no primeiro campo que alguém acrescentasse de um lado
     só, e o editor do diagrama ER já usa exatamente estes dois.

     Os relacionamentos entram DEPOIS de todas as tabelas, por `declarar_fk`, e
     não dentro do `criar_tabela`: assim a ordem de criação deixa de importar,
     e uma FK entre duas tabelas do mesmo plano não depende de qual nasceu
     primeiro. */

  const TIPOS_SIMPLES = new Set([
    "Bool", "Int1", "Int2", "Int4", "Int8", "UInt1", "UInt2", "UInt4", "UInt8",
    "Real4", "Real8", "Date", "Time", "DateTime", "Bin", "Memo", "Uuid",
    "Uuid256", "Sequence",
  ]);

  /** O mesmo vocabulário que `valores.rs` analisa, conferido AQUI antes de
   *  qualquer escrita. Tipo inventado (VARCHAR, TEXT, SERIAL) morre na tela e
   *  não no meio de uma criação pela metade. */
  function conferirTipo(t) {
    const s = String(t == null ? "" : t).trim();
    if (TIPOS_SIMPLES.has(s)) return { ok: true };
    let m = /^Str\((\d+)\)$/.exec(s);
    if (m) return +m[1] >= 1 && +m[1] <= 65535
      ? { ok: true }
      : { ok: false, motivo: `Str(${m[1]}): o tamanho vai de 1 a 65535` };
    m = /^Decimal\((\d+)\s*,\s*(\d+)\)$/.exec(s);
    if (m) {
      if (+m[1] < 1 || +m[1] > 38)
        return { ok: false, motivo: `Decimal: a precisão vai de 1 a 38` };
      if (+m[2] > +m[1])
        return { ok: false, motivo: `Decimal: a escala não passa da precisão` };
      return { ok: true };
    }
    // O motor ACEITA `Str` e `Decimal` sem parâmetro, caindo em Str(60) e
    // Decimal(15,2). Recusar seria mentir sobre o motor; então passa, com o
    // aviso — decisão implícita de tamanho é coisa que se lê, não se descobre.
    if (s === "Str") return { ok: true, aviso: "Str sem tamanho vira Str(60)" };
    if (s === "Decimal")
      return { ok: true, aviso: "Decimal sem parâmetro vira Decimal(15,2)" };
    return { ok: false, motivo: `tipo "${s}" não existe no PhxSql` };
  }

  /** Lê o plano do texto da resposta. Analisa — não recorta por posição. */
  function analisarPlano(texto) {
    let t = String(texto || "").trim();
    // Cerca de código, quando o modelo põe uma mesmo instruído a não pôr.
    const linhas = t.split("\n");
    if (linhas.length > 1 && /^```/.test(linhas[0])) {
      linhas.shift();
      if (/^```/.test(linhas[linhas.length - 1])) linhas.pop();
      t = linhas.join("\n").trim();
    }
    // Um objeto pode vir com prosa em volta: pega do primeiro `{` ao último
    // `}` e deixa o analisador de JSON decidir se aquilo é um objeto. Se não
    // for, não vira plano nenhum — vira recusa com o tamanho, como manda a
    // regra de não fingir que se leu o que não se leu.
    const a = t.indexOf("{"), b = t.lastIndexOf("}");
    if (a >= 0 && b > a) t = t.slice(a, b + 1);
    let j;
    try { j = JSON.parse(t); }
    catch {
      throw new Error("A resposta não veio no formato de plano que esta tela "
        + `sabe conferir (${String(texto || "").length} bytes de texto). `
        + "Peça de novo, ou use um modelo mais capaz em Configurações.");
    }
    if (!j || !Array.isArray(j.tabelas))
      throw new Error("O plano veio sem a lista de tabelas.");
    return j;
  }

  /** Confere o plano contra o motor e contra o que já existe no banco.
   *
   *  Tudo aqui acontece ANTES de a primeira tabela ser criada: um plano com
   *  tipo inexistente não pode virar meia criação. */
  function conferirPlano(plano, existentes) {
    const jaTem = new Set((existentes || []).map(n => String(n).toLowerCase()));
    const doPlano = new Set();
    const itens = [];
    for (const t of plano.tabelas || []) {
      const nome = String(t.nome || "").trim();
      const cols = Array.isArray(t.colunas) ? t.colunas : [];
      const problemas = [];
      const avisos = [];

      if (!nome) problemas.push("tabela sem nome");
      if (!cols.length) problemas.push("tabela sem coluna nenhuma");
      if (jaTem.has(nome.toLowerCase()))
        problemas.push(`a tabela "${nome}" JÁ EXISTE neste banco — e o PhxSql `
          + "não tem ALTER de coluna, então ela não pode ser alterada aqui. "
          + "Nada é sobrescrito: crie com outro nome, ou duplique e recrie.");
      if (doPlano.has(nome.toLowerCase()))
        problemas.push(`o plano traz "${nome}" duas vezes`);
      doPlano.add(nome.toLowerCase());

      const nomesCol = new Set();
      let sequencias = 0;
      for (const c of cols) {
        const cn = String(c.nome || "").trim();
        if (!cn) { problemas.push("coluna sem nome"); continue; }
        if (nomesCol.has(cn.toLowerCase()))
          problemas.push(`a coluna "${cn}" aparece duas vezes`);
        nomesCol.add(cn.toLowerCase());
        const v = conferirTipo(c.tipo);
        if (!v.ok) problemas.push(`coluna "${cn}": ${v.motivo}`);
        else if (v.aviso) avisos.push(`coluna "${cn}": ${v.aviso}`);
        if (String(c.tipo || "").trim() === "Sequence") sequencias++;
      }
      if (sequencias > 1)
        problemas.push("mais de uma coluna Sequence — o PhxSql aceita uma só");

      const indices = Array.isArray(t.indices) ? t.indices : [];
      for (const i of indices) {
        for (const ic of (i.colunas || [])) {
          // O índice aceita `nome desc` e `nome nocase`; a coluna é a primeira
          // palavra, e é ela que precisa existir.
          const so = String(ic).trim().split(/\s+/)[0];
          if (!nomesCol.has(so.toLowerCase()))
            problemas.push(`o índice "${i.nome}" cita a coluna "${so}", `
              + "que a tabela não tem");
        }
      }
      if (!indices.some(i => i.primario))
        avisos.push("sem índice primário — a tabela funciona, mas nada garante "
          + "a unicidade da chave");

      itens.push({ tipo: "tabela", nome, tabela: t, colunas: cols.length,
                   indices: indices.length, problemas, avisos,
                   marcado: problemas.length === 0 });
    }

    // Os relacionamentos, depois: o alvo pode ser uma tabela do próprio plano
    // ou uma que já existe no banco.
    const fks = [];
    for (const t of plano.tabelas || []) {
      for (const f of (t.relacionamentos || t.chaves_estrangeiras || [])) {
        const problemas = [];
        const de = String(t.nome || "").trim();
        const para = String(f.tabela_ref || "").trim();
        const cols = Array.isArray(f.colunas) ? f.colunas : [];
        const nomesCol = new Set((t.colunas || [])
          .map(c => String(c.nome || "").toLowerCase()));
        if (!para) problemas.push("relacionamento sem tabela de destino");
        else if (!doPlano.has(para.toLowerCase()) && !jaTem.has(para.toLowerCase()))
          problemas.push(`a tabela de destino "${para}" não existe nem no plano `
            + "nem neste banco");
        if (!cols.length) problemas.push("relacionamento sem coluna");
        for (const c of cols)
          if (!nomesCol.has(String(c).toLowerCase()))
            problemas.push(`a coluna "${c}" não existe em "${de}"`);
        fks.push({ tipo: "fk", nome: String(f.nome || `fk_${para}`).trim(),
                   de, para, fk: f, problemas, avisos: [],
                   marcado: problemas.length === 0 });
      }
    }
    return { tabelas: itens, fks };
  }

  /* ---------------------------------------------------------- o contexto
     Montado a partir do `esquema` do protocolo, que passa pelo portão de
     permissão normal. Tabela que este usuário não pode ler não entra — e a
     contagem diz quantas ficaram de fora, em vez de fingir que o banco é
     menor do que é. */

  function linhaDaColuna(c) {
    const marcas = [];
    if (c.primaria) marcas.push("PK");
    if (c.estrangeira) marcas.push("FK");
    if (c.dado_pessoal && c.dado_pessoal !== "nao") marcas.push("dado " + c.dado_pessoal);
    return "  " + String(c.nome).padEnd(22) + String(c.tipo).padEnd(22)
         + (c.nullable ? "nulo ok" : "obrigatória")
         + (marcas.length ? "  [" + marcas.join(", ") + "]" : "");
  }

  function textoDoEsquema(e) {
    const l = [`tabela ${e.tabela || ""} (${e.registros} registro(s))`];
    for (const c of e.colunas || []) {
      if (c.sistema) continue;           // rownum e companhia não ajudam o modelo
      l.push(linhaDaColuna(c));
    }
    for (const i of e.indices || []) {
      const cols = (i.colunas || []).map(x => x.coluna + (x.desc ? " desc" : "")).join(", ");
      l.push(`  índice ${i.nome} (${cols})${i.unico ? " único" : ""}${i.primario ? " primário" : ""}`);
    }
    for (const f of e.chaves_estrangeiras || []) {
      l.push(`  fk ${f.nome} (${(f.colunas || []).join(", ")}) -> `
           + `${f.tabela_ref}(${(f.colunas_ref || []).join(", ")})`
           + ` ao excluir: ${f.ao_excluir} · ao alterar: ${f.ao_alterar}`);
    }
    return l.join("\n");
  }

  /** Redige a linha por ANÁLISE: percorre as colunas do esquema e troca o
   *  valor das marcadas como dado pessoal. Recortar o texto do JSON dependeria
   *  de ele estar escrito de um jeito; analisar e reserializar, não. */
  function redigir(linha, colunas) {
    const fora = new Set((colunas || [])
      .filter(c => c.dado_pessoal && c.dado_pessoal !== "nao")
      .map(c => c.nome));
    const saida = {};
    let redigidas = 0;
    for (const k of Object.keys(linha)) {
      if (fora.has(k)) { saida[k] = "***"; redigidas++; }
      else saida[k] = linha[k];
    }
    return { linha: saida, redigidas };
  }

  async function montarContexto(db, escolha) {
    const partes = [];
    const resumo = { tabelas: 0, fora: 0, linhas: 0, redigidas: 0, semBanco: false };
    if (!escolha.esquema) return { texto: "", resumo };
    // Pedir o esquema sem dizer de qual banco não pode virar «sobe nada» sem
    // recado: a resposta viria chutando nomes de coluna e ninguém saberia que
    // a pergunta foi feita no escuro.
    if (!db) { resumo.semBanco = true; return { texto: "", resumo }; }

    const t = await api("tabelas", { database: db });
    const nomes = t.tabelas || [];
    partes.push(`Banco "${db}", ${nomes.length} tabela(s).`);
    for (const nome of nomes) {
      let e;
      try { e = await api("esquema", { database: db, tabela: nome }); }
      catch { resumo.fora++; continue; }   // sem permissão: fica de fora
      e.tabela = nome;
      partes.push(textoDoEsquema(e));
      resumo.tabelas++;

      if (escolha.linhas > 0) {
        try {
          const r = await api("varrer", { database: db, tabela: nome, max: escolha.linhas });
          const amostra = [];
          for (const l of (r.linhas || [])) {
            const x = redigir(l, e.colunas);
            resumo.redigidas += x.redigidas;
            amostra.push(x.linha);
          }
          if (amostra.length) {
            resumo.linhas += amostra.length;
            partes.push(`  linhas de exemplo de ${nome}:\n`
              + amostra.map(a => "    " + JSON.stringify(a)).join("\n"));
          }
        } catch { /* sem permissão de ler linha: segue sem amostra */ }
      }
    }
    if (resumo.fora)
      partes.push(`(${resumo.fora} tabela(s) ficaram de fora: sem permissão de leitura.)`);
    return { texto: partes.join("\n\n"), resumo };
  }

  /* --------------------------------------------------------------- telas */

  const E = s => (window.esc ? esc(s) : String(s));

  /** A tela de Configurações → Integração com a Claude. */
  function telaConfig() {
    const c = cfg();
    const temChave = !!c.chave;
    folha("Integração com a Claude",
      "a chave é sua e fica neste navegador · o servidor PhxSql não participa",
      `<div class="aviso">
         <b>Leia antes de ligar.</b> Esta tela liga o Centro de Controle
         direto na API da Anthropic, <b>do seu navegador</b>. Em português
         claro:
         <ul class="lista-limpa" style="margin-top:8px">
           <li>· a chave fica guardada <b>neste navegador</b> (no
               <code>localStorage</code>), e não no servidor — quem usar o
               console de outra máquina precisa da própria chave;</li>
           <li>· as suas perguntas e o contexto que você mandar
               (o <b>esquema</b> do banco, e as linhas se você marcar)
               <b>vão para a Anthropic</b>, que é uma empresa de fora;</li>
           <li>· o servidor PhxSql <b>não participa</b>: ele nunca vê a chave,
               nunca faz a chamada e não guarda nada disto;</li>
           <li>· o <b>custo é seu</b>, na sua conta da Anthropic. Cada
               resposta mostra os tokens que consumiu.</li>
         </ul>
       </div>

       <div class="form-dbl">
         <label class="cmp"><span>Chave da API</span>
           <input id="iaChave" type="password" autocomplete="off"
                  placeholder="${temChave ? "guardada — digite para trocar" : "sk-ant-…"}">
           <span class="leg">${temChave
             ? `Há uma chave guardada, terminada em <code>${E(fim(c.chave))}</code>.`
             : "Ainda não há chave guardada neste navegador."}</span></label>

         <label class="cmp"><span>Modelo</span>
           <select id="iaModelo">${MODELOS.map(m =>
             `<option value="${m.id}"${m.id === c.modelo ? " selected" : ""}
               >${E(m.rot)} — ${E(m.diz)}</option>`).join("")}</select>
           <span class="leg">A escolha é de <b>custo</b>: os três respondem, e
             o mais capaz cobra mais por token.</span></label>

         <label class="cmp linha-chk">
           <input id="iaLigado" type="checkbox"${c.ligado ? " checked" : ""}>
           <span>Ligada — mostrar os botões da Claude na tela de Query</span></label>

         <div class="cmp linha-chk">
           <span class="leg">Endereço da API:
             <code>${E(c.endpoint || ENDPOINT_OFICIAL)}</code>
             ${oficial(c) ? `<span class="pino ok">oficial</span>`
                          : `<span class="pino mal">NÃO é o oficial</span>`}</span></div>
       </div>

       <div class="dbl-titulo" style="margin-top:16px">
         <button class="botao incluir" id="iaSalvar">Salvar</button>
         <button class="botao consultar" id="iaTestar">Testar a chave</button>
         <button class="botao excluir" id="iaRemover"
                 ${temChave ? "" : "disabled"}>Remover a chave deste navegador</button>
         <span class="cresce"></span>
       </div>
       <div id="iaRecado"></div>

       <div class="nota" style="margin-top:18px">
         <p>O desenho e o porquê estão em <code>docs/CLAUDE-IA.md</code>.
         Em uma frase: a API é HTTPS obrigatório, a <code>std</code> do Rust
         não tem TLS, e a casa não acrescenta dependência — então a chamada
         sai do navegador, e o servidor fica de fora do caminho inteiro.</p>
       </div>`);

    const recado = (txt, classe) => {
      $("#iaRecado").innerHTML = `<div class="aviso ${classe}">${txt}</div>`;
    };

    $("#iaSalvar").onclick = () => {
      const nova = $("#iaChave").value.trim();
      const m = { modelo: $("#iaModelo").value, ligado: $("#iaLigado").checked };
      if (nova) m.chave = nova;
      gravar(m);
      avisar("integração com a Claude salva neste navegador");
      telaConfig();
    };

    $("#iaRemover").onclick = () => {
      gravar({ chave: "", ligado: false });
      avisar("chave removida deste navegador");
      telaConfig();
    };

    $("#iaTestar").onclick = async () => {
      const nova = $("#iaChave").value.trim();
      if (nova) gravar({ chave: nova });
      if (!cfg().chave) return recado("Não há chave para testar.", "mal");
      recado("testando…", "");
      try {
        // A chamada mínima que prova a chave: uma palavra de entrada e um
        // teto de saída pequeno. Custa quase nada e responde a única pergunta
        // que interessa — a chave é aceita?
        const r = await perguntar(
          { sistema: "Responda apenas: ok" }, "ok", "", null);
        recado(`A chave funciona. A API respondeu «${E(r.texto.trim().slice(0, 40))}» `
          + `· ${r.uso.entrada} token(s) de entrada, ${r.uso.saida} de saída.`, "bom");
      } catch (e) {
        recado(E(e.message || String(e)), "mal");
      }
    };
  }

  /* ------------------------------------------- o painel da tela de Query */

  /** Desenha o botão da Claude na tela de Query.
   *
   *  Sem integração ligada, NÃO desenha nada: a tela de Query fica exatamente
   *  como era antes desta rodada. É o comportamento velho, e é o que o teste
   *  trava — guarda nova entra pedida, não imposta. */
  function botaoDaConsulta(alvo) {
    if (!ligada()) return false;
    const d = document.createElement("div");
    d.className = "dbl-titulo";
    d.style.marginTop = "14px";
    d.innerHTML = `<button class="botao consultar" id="btIA">✦ Perguntar à Claude</button>
      <span class="leg">o SQL gerado <b>não executa sozinho</b> — ele cai no
        editor abaixo, e quem aperta Executar é você</span>`;
    alvo.appendChild(d);
    const painel = document.createElement("div");
    painel.id = "iaPainel";
    alvo.appendChild(painel);
    d.querySelector("#btIA").onclick = () => painelDaConsulta(painel);
    return true;
  }

  let receitaAtual = "sql";

  /** Qual banco vira contexto.
   *
   *  A tela de Query se abre sem tabela escolhida, e aí `est.atual` é nulo.
   *  Deixar o campo vazio faria o contexto sair VAZIO em silêncio — a
   *  pergunta iria sem esquema nenhum e a resposta chutaria nomes de coluna,
   *  sem ninguém entender por quê. Então cai para o banco corrente e, na
   *  falta dele, para o primeiro que o usuário enxerga. */
  async function bancoDoContexto() {
    if (est.atual && est.atual.db) return est.atual.db;
    if (est.database) return est.database;
    try {
      const bancos = est.bancos || await api("bancos");
      return (bancos && bancos[0]) || "";
    } catch { return ""; }
  }

  async function painelDaConsulta(onde) {
    const db = await bancoDoContexto();
    const c = cfg();
    onde.innerHTML = `
      <div class="dbl-titulo" style="margin-top:14px">
        ${Object.entries(RECEITAS).map(([k, r]) =>
          `<button class="botao mini consultar ia-rec" data-r="${k}"
            ${k === receitaAtual ? 'style="background:var(--acao-consultar);color:var(--fundo)"' : ""}
            >${r.ico} ${E(r.rot)}</button>`).join("")}
        <span class="cresce"></span>
        <span class="leg">modelo: <code>${E(c.modelo || MODELO_PADRAO)}</code></span>
      </div>
      <div id="iaCorpo"></div>`;
    for (const b of onde.querySelectorAll(".ia-rec"))
      b.onclick = () => { receitaAtual = b.dataset.r; painelDaConsulta(onde); };
    desenharReceita(onde.querySelector("#iaCorpo"), db);
  }

  function desenharReceita(onde, db) {
    const r = RECEITAS[receitaAtual];
    onde.innerHTML = `
      <div class="form-dbl">
        <label class="cmp" style="grid-column:1/-1"><span>${E(r.pede)}</span>
          <textarea id="iaPergunta" rows="3" style="width:100%;padding:8px 10px;
            border:1px solid var(--linha);border-radius:5px;background:var(--painel);
            color:var(--texto);font-size:12.5px;font-family:'IBM Plex Mono',monospace;
            resize:vertical" placeholder="${E(r.exemplo)}"></textarea></label>

        <label class="cmp"><span>Database do contexto</span>
          <input id="iaDb" value="${E(db || "")}"
                 ${r.esquema ? "" : "disabled"}></label>

        <label class="cmp linha-chk">
          <input id="iaEsq" type="checkbox"${r.esquema ? " checked" : ""}
                 ${r.esquema ? "" : "disabled"}>
          <span>Mandar o <b>esquema</b> deste banco (nomes de tabela, de coluna,
            tipos, chaves) — é o que faz a resposta acertar</span></label>

        <label class="cmp linha-chk">
          <input id="iaLinhas" type="checkbox">
          <span>Mandar também <b>linhas de exemplo</b> de cada tabela</span></label>

        <div class="cmp linha-chk" style="grid-column:1/-1">
          <span class="leg" id="iaLinhasQtd" hidden>quantas por tabela:
            <input id="iaQtd" type="number" value="3" min="1" max="20"
                   style="width:70px;display:inline-block"></span></div>
      </div>

      <div id="iaAlertaLinhas"></div>

      <div class="dbl-titulo" style="margin-top:12px">
        <button class="botao consultar" id="iaVer">Ver o que vai subir</button>
        <button class="botao incluir" id="iaIr">Perguntar</button>
        <span class="cresce"></span>
        <span class="leg" id="iaTokens"></span>
      </div>

      <div id="iaEnvio"></div>
      <div id="iaSaida"></div>
      ${r.editor ? `
        <h3>Editor — o SQL cai aqui, e quem executa é você</h3>
        <textarea id="iaSql" rows="4" style="width:100%;padding:10px 12px;
          border:1px solid var(--linha);border-radius:6px;background:var(--painel);
          color:var(--texto);font-size:12.5px;font-family:'IBM Plex Mono',monospace;
          resize:vertical"></textarea>
        <div class="dbl-titulo" style="margin-top:10px">
          <button class="botao consultar" id="iaExecutar">Executar</button>
          <span class="leg">nada roda sem este clique</span>
        </div>
        <div id="iaResultado"></div>` : ""}`;

    const chk = onde.querySelector("#iaLinhas");
    chk.onchange = () => {
      onde.querySelector("#iaLinhasQtd").hidden = !chk.checked;
      onde.querySelector("#iaAlertaLinhas").innerHTML = chk.checked
        ? `<div class="aviso mal"><b>O dado sai desta máquina.</b> Marcada, esta
             caixa manda linhas de verdade das tabelas para a Anthropic. Coluna
             marcada como <b>dado pessoal</b> no esquema vai redigida
             (<code>"***"</code>), mas o resto vai como está. Marque só se você
             pode fazer isso com este banco.</div>`
        : "";
    };

    onde.querySelector("#iaVer").onclick = () => mostrarEnvio(onde);
    onde.querySelector("#iaIr").onclick = () => ir(onde);
    if (r.editor) onde.querySelector("#iaExecutar").onclick = () => executar(onde);
  }

  function escolhaDe(onde) {
    return {
      esquema: onde.querySelector("#iaEsq").checked,
      linhas: onde.querySelector("#iaLinhas").checked
        ? (+onde.querySelector("#iaQtd").value || 3) : 0,
      db: onde.querySelector("#iaDb").value.trim(),
      pergunta: onde.querySelector("#iaPergunta").value.trim(),
    };
  }

  /** O painel «o que vai subir»: o corpo EXATO do POST, e os cabeçalhos com a
   *  chave mascarada. Mostrar o corpo sem os cabeçalhos esconderia justamente
   *  a parte que é segredo. */
  async function mostrarEnvio(onde, jaMontado) {
    const e = jaMontado || escolhaDe(onde);
    const alvo = onde.querySelector("#iaEnvio");
    alvo.innerHTML = `<div class="centro">montando o contexto…</div>`;
    let ctx;
    try { ctx = jaMontado ? jaMontado.ctx : await montarContexto(e.db, e); }
    catch (err) {
      alvo.innerHTML = `<div class="aviso mal">Não deu para ler o esquema: ${E(String(err))}</div>`;
      return null;
    }
    const c = cfg();
    const b = corpo(RECEITAS[receitaAtual], e.pergunta || "(ainda vazio)",
                    ctx.texto, c.modelo || MODELO_PADRAO);
    const cab = Object.assign({}, cabecalhos(c.chave));
    cab["x-api-key"] = fim(c.chave) || "(sem chave)";
    alvo.innerHTML = (ctx.resumo.semBanco
      ? `<div class="aviso mal">Você pediu para mandar o esquema, mas nenhum
           database está escolhido — então <b>nenhum esquema vai subir</b> e a
           resposta vai chutar os nomes das colunas. Escreva o database no
           campo acima.</div>`
      : "") + `
      <details class="nota" open>
        <summary><b>O que vai subir para a Anthropic</b> —
          ${ctx.resumo.tabelas} tabela(s) de esquema,
          ${ctx.resumo.linhas} linha(s) de exemplo,
          ${ctx.resumo.redigidas} valor(es) redigido(s) por serem dado pessoal.</summary>
        <p class="leg">POST <code>${E(c.endpoint || ENDPOINT_OFICIAL)}</code></p>
        <pre class="dado" style="white-space:pre-wrap;word-break:break-word;overflow-x:auto">${E(JSON.stringify(cab, null, 1))}</pre>
        <pre class="dado" style="white-space:pre-wrap;word-break:break-word;overflow-x:auto">${E(JSON.stringify(b, null, 1))}</pre>
      </details>`;
    return ctx;
  }

  async function ir(onde) {
    const e = escolhaDe(onde);
    const saida = onde.querySelector("#iaSaida");
    if (!e.pergunta) {
      saida.innerHTML = `<div class="aviso mal">Escreva a pergunta primeiro.</div>`;
      return;
    }
    saida.innerHTML = `<div class="centro">montando o contexto…</div>`;
    let ctx;
    try { ctx = await montarContexto(e.db, e); }
    catch (err) {
      saida.innerHTML = `<div class="aviso mal">Não deu para ler o esquema: ${E(String(err))}</div>`;
      return;
    }
    // O painel do que subiu fica montado ANTES de a resposta chegar: quem
    // quiser conferir não precisa esperar, e não descobre depois.
    await mostrarEnvio(onde, Object.assign({}, e, { ctx }));

    saida.innerHTML = `<h3>Resposta</h3><pre class="dado" id="iaTexto" style="white-space:pre-wrap;word-break:break-word">…</pre>`;
    const alvo = onde.querySelector("#iaTexto");
    const tok = onde.querySelector("#iaTokens");
    try {
      const r = await perguntar(RECEITAS[receitaAtual], e.pergunta, ctx.texto,
        (parcial, uso) => {
          alvo.textContent = parcial;
          tok.innerHTML = `entrada ${uso.entrada} · saída ${uso.saida} token(s)`;
        });
      alvo.textContent = r.texto;
      tok.innerHTML = `entrada <b>${r.uso.entrada}</b> · saída <b>${r.uso.saida}</b>
        token(s) — o custo é da sua conta`;
      if (RECEITAS[receitaAtual].editor) {
        const campo = onde.querySelector("#iaSql");
        if (campo) campo.value = limparSql(r.texto);
      }
      // O plano vira REVISÃO, e não criação: a conferência e os cliques ficam
      // entre a resposta e a primeira escrita.
      if (RECEITAS[receitaAtual].plano)
        await renderizarPlano(onde, r.texto, e.db);
    } catch (err) {
      saida.innerHTML = `<div class="aviso mal">${E(err.message || String(err))}</div>`;
    }
  }

  /* ================================================ a revisão e a criação
     A IA PROPÕE; quem cria é a pessoa que confirma. É a mesma linha do SQL
     gerado, e ela não se cruza: nada é gravado sem um clique consciente, e a
     tela diz quantas tabelas e quantas colunas serão criadas ANTES de a
     primeira escrita acontecer. */

  /** O que esta rodada criou, para o desfazer saber o que remover. */
  let nascidos = { db: "", tabelas: [], fks: [] };

  async function renderizarPlano(onde, texto, db) {
    const alvo = onde.querySelector("#iaSaida");
    let plano, conf;
    try {
      plano = analisarPlano(texto);
      const t = await api("tabelas", { database: db });
      conf = conferirPlano(plano, t.tabelas || []);
    } catch (e) {
      alvo.innerHTML = `<div class="aviso mal">${E(e.message || String(e))}</div>`
        + `<h3>O que a Claude respondeu</h3><pre class="dado" style="white-space:pre-wrap;word-break:break-word;overflow-x:auto">${E(texto)}</pre>`;
      return;
    }
    desenharRevisao(onde, conf, plano, db);
  }

  function desenharRevisao(onde, conf, plano, db) {
    const alvo = onde.querySelector("#iaSaida");
    const boas = conf.tabelas.filter(i => !i.problemas.length);
    const ruins = conf.tabelas.filter(i => i.problemas.length);
    const fksBoas = conf.fks.filter(i => !i.problemas.length);
    const fksRuins = conf.fks.filter(i => i.problemas.length);

    const linhaTabela = (i, n) => `
      <div class="ia-item" style="border:1px solid var(--linha);border-radius:6px;
           padding:10px 12px;margin:8px 0;background:var(--painel)">
        <label class="linha-chk" style="display:flex;gap:8px;align-items:center;
               text-transform:none;letter-spacing:0">
          <input type="checkbox" class="ia-mt" data-i="${n}" style="width:auto"
                 ${i.marcado ? "checked" : ""} ${i.problemas.length ? "disabled" : ""}>
          <b>${E(i.nome)}</b>
          <span class="pino">${i.colunas} coluna(s)</span>
          <span class="pino">${i.indices} índice(s)</span>
        </label>
        ${i.tabela.porque ? `<p class="leg">${E(i.tabela.porque)}</p>` : ""}
        <div style="overflow-x:auto"><table><thead><tr><th>coluna</th><th>tipo</th>
          <th>obrig.</th><th>dado pessoal</th></tr></thead><tbody>
          ${(i.tabela.colunas || []).map(c => `<tr>
            <td class="dado">${E(c.nome)}</td>
            <td class="dado">${E(c.tipo)}</td>
            <td>${c.obrigatoria ? '<span class="pino ok">sim</span>'
                                : '<span class="pino nao">não</span>'}</td>
            <td>${c.dado_pessoal && c.dado_pessoal !== "nao"
                  ? `<span class="pino mal">${E(c.dado_pessoal)}</span>`
                  : '<span class="pino nao">não</span>'}</td></tr>`).join("")}
        </tbody></table></div>
        ${(i.tabela.indices || []).map(x => `<p class="leg">índice
           <code>${E(x.nome)}</code> (${E((x.colunas || []).join(", "))})
           ${x.unico ? "único" : ""} ${x.primario ? "primário" : ""}
           ${x.porque ? "— " + E(x.porque) : ""}</p>`).join("")}
        ${i.avisos.map(a => `<div class="aviso">${E(a)}</div>`).join("")}
        ${i.problemas.map(pr => `<div class="aviso mal">${E(pr)}</div>`).join("")}
      </div>`;

    alvo.innerHTML = `
      <h3>Revisão do plano — nada foi criado ainda</h3>
      <div class="aviso"><b>A Claude propôs; quem cria é você.</b> Confira item
        por item e desmarque o que não quiser. Ao confirmar, o PhxSql vai criar
        <b id="iaConta">${boas.length} tabela(s)</b> e
        <b id="iaContaFk">${fksBoas.length} relacionamento(s)</b>
        no banco <code>${E(db)}</code>.</div>
      ${ruins.length ? `<div class="aviso mal">${ruins.length} tabela(s) do plano
        <b>não podem ser criadas</b> e ficaram travadas abaixo, com o motivo.
        Nada é sobrescrito em silêncio.</div>` : ""}
      ${(plano.notas || []).length
        ? `<div class="nota">${(plano.notas || []).map(n =>
            `<p>${E(n)}</p>`).join("")}</div>` : ""}
      ${conf.tabelas.map(linhaTabela).join("")}
      ${conf.fks.length ? `<h3>Relacionamentos</h3>
        <div class="aviso"><b>A chave estrangeira do PhxSql é DECLARADA, e não
          imposta.</b> O motor guarda a declaração e as telas a usam — mas ele
          <b>não confere</b> a referência na hora de gravar. Quem promete
          integridade que não existe entrega um estrago com nome bonito.</div>
        ${conf.fks.map((f, n) => `
          <div style="border:1px solid var(--linha);border-radius:6px;
               padding:10px 12px;margin:8px 0;background:var(--painel)">
            <label class="linha-chk" style="display:flex;gap:8px;align-items:center;
                   text-transform:none;letter-spacing:0">
              <input type="checkbox" class="ia-mf" data-i="${n}" style="width:auto"
                     ${f.marcado ? "checked" : ""} ${f.problemas.length ? "disabled" : ""}>
              <code>${E(f.de)}</code>(${E((f.fk.colunas || []).join(", "))})
              → <code>${E(f.para)}</code>(${E((f.fk.colunas_ref || []).join(", "))})
              <span class="pino">ao excluir: ${E(f.fk.ao_excluir || "restringir")}</span>
            </label>
            ${f.fk.porque ? `<p class="leg">${E(f.fk.porque)}</p>` : ""}
            ${f.problemas.map(pr => `<div class="aviso mal">${E(pr)}</div>`).join("")}
          </div>`).join("")}` : ""}
      ${fksRuins.length ? "" : ""}
      <div class="dbl-titulo" style="margin-top:14px">
        <button class="botao incluir" id="iaCriar">Criar o que está marcado</button>
        <span class="leg">só este clique escreve no banco</span>
      </div>
      <div id="iaNascido"></div>`;

    const conta = () => {
      onde.querySelector("#iaConta").textContent =
        `${[...onde.querySelectorAll(".ia-mt")].filter(x => x.checked).length} tabela(s)`;
      onde.querySelector("#iaContaFk").textContent =
        `${[...onde.querySelectorAll(".ia-mf")].filter(x => x.checked).length} relacionamento(s)`;
    };
    for (const c of onde.querySelectorAll(".ia-mt, .ia-mf")) c.onchange = conta;
    onde.querySelector("#iaCriar").onclick = () => criarDoPlano(onde, conf, db);
  }

  /** Executa o plano confirmado, pelas operações que já existem. */
  async function criarDoPlano(onde, conf, db) {
    const alvo = onde.querySelector("#iaNascido");
    const marcadas = [...onde.querySelectorAll(".ia-mt")]
      .filter(x => x.checked).map(x => conf.tabelas[+x.dataset.i]);
    const marcadasFk = [...onde.querySelectorAll(".ia-mf")]
      .filter(x => x.checked).map(x => conf.fks[+x.dataset.i]);
    if (!marcadas.length && !marcadasFk.length) {
      alvo.innerHTML = `<div class="aviso mal">Nada marcado.</div>`;
      return;
    }
    alvo.innerHTML = `<div class="centro">criando…</div>`;
    nascidos = { db, tabelas: [], fks: [] };
    const feitos = [];

    for (const i of marcadas) {
      try {
        await api("criar_tabela", {
          database: db, tabela: i.nome,
          colunas: (i.tabela.colunas || []).map(c => ({
            nome: c.nome, tipo: c.tipo, obrigatoria: !!c.obrigatoria,
            caption: c.caption || "", dado_pessoal: c.dado_pessoal || "nao",
          })),
          indices: (i.tabela.indices || []).map(x => ({
            nome: x.nome, colunas: x.colunas, unico: !!x.unico,
            primario: !!x.primario,
          })),
        });
        nascidos.tabelas.push(i.nome);
        feitos.push([true, `tabela <b>${E(i.nome)}</b> criada com `
          + `${(i.tabela.colunas || []).length} coluna(s)`]);
      } catch (e) {
        feitos.push([false, `tabela <b>${E(i.nome)}</b>: ${E(String(e.message || e))}`]);
      }
    }
    // Os relacionamentos por último: assim a ordem de criação das tabelas não
    // importa, e uma FK entre duas do mesmo plano sempre acha o destino.
    for (const f of marcadasFk) {
      try {
        await api("declarar_fk", {
          database: db, tabela: f.de, nome: f.nome,
          colunas: f.fk.colunas, tabela_ref: f.para,
          colunas_ref: f.fk.colunas_ref || f.fk.colunas,
          ao_excluir: f.fk.ao_excluir || "restringir",
          ao_alterar: f.fk.ao_alterar || "restringir",
        });
        nascidos.fks.push({ tabela: f.de, nome: f.nome });
        feitos.push([true, `relacionamento <b>${E(f.nome)}</b>: `
          + `${E(f.de)} → ${E(f.para)} declarado`]);
      } catch (e) {
        feitos.push([false, `relacionamento <b>${E(f.nome)}</b>: `
          + `${E(String(e.message || e))}`]);
      }
    }

    const bons = feitos.filter(x => x[0]).length;
    alvo.innerHTML = `
      <h3>O que nasceu</h3>
      <div class="aviso ${bons === feitos.length ? "bom" : "mal"}">
        ${bons} de ${feitos.length} item(ns) criados.</div>
      <ul class="lista-limpa">${feitos.map(([o, t]) =>
        `<li>${o ? "·" : "<span class='mal'>×</span>"} ${t}</li>`).join("")}</ul>
      <div class="dbl-titulo" style="margin-top:12px">
        <button class="botao consultar" id="iaVerDic">Dicionário de dados</button>
        <button class="botao consultar" id="iaVerEr">Diagrama ER em tela cheia</button>
        <button class="botao excluir" id="iaDesfazer">Desfazer esta rodada</button>
      </div>
      <div id="iaDesfeito"></div>
      <h3>O modelo agora</h3>
      <div class="er-rolo" id="iaEr"><div class="centro">desenhando…</div></div>`;

    onde.querySelector("#iaVerDic").onclick = () => verSysColumns(db);
    onde.querySelector("#iaVerEr").onclick = () => telaDiagramaER(db);
    onde.querySelector("#iaDesfazer").onclick = () => desfazer(onde, db);
    await desenharModeloAgora(onde, db);
    // A árvore da esquerda também tem de ver o que nasceu, senão a tela mostra
    // dois estados do mesmo banco.
    // `montarArvore()` sem argumento ABRE O PAINEL e joga a pessoa para fora da
    // tela em que ela está -- o padrão do parâmetro é `true`. Só o exercício
    // mostrou isso: a revisão inteira sumia no instante da criação.
    try { await montarArvore(false); } catch { /* a árvore é enfeite aqui */ }
  }

  /** O diagrama do estado REAL do banco, relido do servidor.
   *
   *  Reusa o `PhxER` do editor de diagrama em vez de inventar uma segunda
   *  visualização — e relê do servidor em vez de desenhar o plano, porque o
   *  que interessa mostrar é o que existe, e não o que foi pedido. */
  async function desenharModeloAgora(onde, db) {
    const alvo = onde.querySelector("#iaEr");
    if (!alvo) return;
    try {
      const t = await api("tabelas", { database: db });
      const esquemas = [];
      for (const nome of (t.tabelas || [])) {
        try { esquemas.push(await api("esquema", { database: db, tabela: nome })); }
        catch { /* sem permissão: fica de fora, como no diagrama */ }
      }
      if (!esquemas.length) { alvo.innerHTML = `<div class="vazio">sem tabelas</div>`; return; }
      alvo.innerHTML = "";
      PhxER.montar(alvo, esquemas, { aoAbrir: () => {} });
      const r = PhxER.resumo(esquemas);
      alvo.insertAdjacentHTML("afterend",
        `<p class="leg">${r.tabelas} tabela(s) · ${r.ligacoes} relacionamento(s)
         · ${r.sem_ligacao} sem ligação</p>`);
    } catch (e) {
      alvo.innerHTML = `<div class="aviso mal">${E(String(e.message || e))}</div>`;
    }
  }

  /** Remove o que ESTA rodada criou, e só isso.
   *
   *  Tabela recém-criada e vazia é o caso fácil. Tabela que já ganhou linha
   *  segue a regra normal de exclusão: o aviso aparece e o clique é outro —
   *  não há atalho para apagar dado por causa de um desfazer. */
  async function desfazer(onde, db) {
    const alvo = onde.querySelector("#iaDesfeito");
    if (!nascidos.tabelas.length && !nascidos.fks.length) {
      alvo.innerHTML = `<div class="aviso">Nada desta rodada para desfazer.</div>`;
      return;
    }
    alvo.innerHTML = `<div class="centro">conferindo…</div>`;
    const comDado = [];
    for (const nome of nascidos.tabelas) {
      try {
        const e = await api("esquema", { database: db, tabela: nome });
        if ((e.registros || 0) > 0) comDado.push([nome, e.registros]);
      } catch { /* sumiu: nada a desfazer */ }
    }
    if (comDado.length && !alvo.dataset.confirmado) {
      alvo.dataset.confirmado = "1";
      alvo.innerHTML = `<div class="aviso mal">
        <b>Atenção: já há dado gravado.</b>
        ${comDado.map(([n, q]) => `<code>${E(n)}</code> tem ${q} linha(s)`).join(", ")}.
        Desfazer apaga a tabela e o dado junto, e não há volta. Clique em
        <b>Desfazer esta rodada</b> outra vez para confirmar.</div>`;
      return;
    }
    const feitos = [];
    for (const f of nascidos.fks) {
      try { await api("excluir_fk", { database: db, tabela: f.tabela, nome: f.nome });
            feitos.push([true, `relacionamento ${E(f.nome)} removido`]); }
      catch (e) { feitos.push([false, `${E(f.nome)}: ${E(String(e.message || e))}`]); }
    }
    for (const nome of nascidos.tabelas) {
      try {
        await api("excluir_tabela", { database: db, tabela: nome, confirmar: nome });
        feitos.push([true, `tabela ${E(nome)} removida`]);
      } catch (e) {
        feitos.push([false, `${E(nome)}: ${E(String(e.message || e))}`]);
      }
    }
    nascidos = { db, tabelas: [], fks: [] };
    delete alvo.dataset.confirmado;
    alvo.innerHTML = `<div class="aviso">${feitos.map(([o, t]) =>
      `${o ? "·" : "×"} ${t}`).join("<br>")}</div>`;
    await desenharModeloAgora(onde, db);
    try { await montarArvore(false); } catch { /* enfeite */ }
  }

  /** Tira a cerca de código que o modelo às vezes põe mesmo instruído a não
   *  pôr. Analisa a estrutura da cerca em vez de recortar por posição. */
  function limparSql(t) {
    const linhas = String(t).trim().split("\n");
    if (linhas.length > 1 && /^```/.test(linhas[0])) {
      linhas.shift();
      if (/^```/.test(linhas[linhas.length - 1])) linhas.pop();
    }
    return linhas.join("\n").trim();
  }

  /** Executa o SQL do editor — pelo clique da pessoa, e nunca sozinho.
   *  Vai pela operação `sql` do protocolo, que passa pelo mesmo portão de
   *  permissão de qualquer outro pedido. */
  async function executar(onde) {
    const texto = onde.querySelector("#iaSql").value.trim();
    const alvo = onde.querySelector("#iaResultado");
    if (!texto) { alvo.innerHTML = `<div class="aviso mal">O editor está vazio.</div>`; return; }
    alvo.innerHTML = `<div class="centro">executando…</div>`;
    try {
      const r = await api("sql", {
        database: onde.querySelector("#iaDb").value.trim(), texto });
      const linhas = r.linhas || [];
      const cols = r.colunas && r.colunas.length
        ? r.colunas : (linhas.length ? Object.keys(linhas[0]) : []);
      alvo.innerHTML =
        `<p class="leg">operação <code>${E(r.op || "?")}</code> ·
          ${r.devolvidas ?? linhas.length} linha(s)
          ${r.contagem !== undefined ? ` · contagem ${r.contagem}` : ""}</p>`
        + ((r.notas || []).length
            ? `<div class="aviso">${(r.notas || []).map(E).join("<br>")}</div>` : "")
        + (linhas.length
            ? tabela(cols.map(x => ({ t: x })), linhas,
                l => `<tr>${cols.map(x => celulaValor(l[x])).join("")}</tr>`)
            : `<div class="vazio">sem linhas</div>`);
    } catch (e) {
      alvo.innerHTML = `<div class="aviso mal">${E(String(e.message || e))}</div>`;
    }
  }

  return {
    telaConfig, botaoDaConsulta, ligada,
    // Expostos para o exercício automatizado poder olhar por dentro sem
    // depender do desenho da tela.
    _cfg: cfg, _corpo: corpo, _cabecalhos: cabecalhos, _limparSql: limparSql,
    _redigir: redigir, _conferirTipo: conferirTipo, _analisarPlano: analisarPlano,
    _conferirPlano: conferirPlano, ENDPOINT_OFICIAL, CABECALHO_NAVEGADOR,
  };
})();
