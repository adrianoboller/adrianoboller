/* Reposicao de defeito SEM recompilar o `phxsqld`.
 *
 * O binario desta rodada (08/09) embute a tela por `include_str!` -- mexer em
 * `ui/claude.js` no disco nao muda UMA linha do que ele serve, e esta frente
 * esta proibida de rodar `cargo`. Repor um defeito para a prova real "falha
 * com o defeito, passa com o conserto" exige entao um caminho que nao passe
 * pelo binario.
 *
 * O caminho: o PROPRIO Playwright ja fica no meio de toda resposta -- e
 * `route.fulfill` deixa reescrever corpo e cabecalhos de UMA resposta
 * especifica antes de o navegador ve-la, com o SERVIDOR DE VERDADE
 * respondendo por tras (login, banco, tudo funciona igual). Isto e' "uma
 * copia do claude.js servida", so que a copia nasce da resposta real, editada
 * no ar -- em vez de um segundo servidor estatico que teria que reimplementar
 * o app inteiro (login, arvore, protocolo) so para pintar duas telas.
 *
 * Os dois defeitos que este arquivo sabe repor sao os dois primeiros da
 * tabela "Os defeitos repostos" da SS8 de `docs/CLAUDE-IA.md`:
 *
 *   1. `connect-src 'self'` sozinho (o cabecalho HTTP da PAGINA)
 *   2. "criar do plano sem confirmacao" (uma linha do corpo do `claude.js`)
 *
 * O terceiro par de teste real desta rodada (chave no corpo de um pedido ao
 * PhxSql) mexe no mesmo corpo por outro pedaco -- ver `PATCH_CHAVE_NO_CORPO`.
 */

/** Intercepta a resposta do documento principal (`GET /` ou `/index.html`) e
 *  deixa reescrever o CORPO e/ou o cabecalho `content-security-policy` antes
 *  de o navegador executar a pagina.
 *
 *  `patchCorpo(texto) => texto` -- recebe o HTML/JS servido de verdade
 *  (com o `claude.js` embutido dentro) e devolve o texto que o navegador vai
 *  ver. `patchCsp(valorAtual) => novoValor` faz o mesmo para o cabecalho.
 *  Os dois sao opcionais; sem nenhum, a rota so repassa a resposta real. */
export async function interceptarPaginaPrincipal(contexto, { patchCorpo, patchCsp } = {}) {
  await contexto.route(url => {
    try { return new URL(url).pathname === '/' || new URL(url).pathname === '/index.html'; }
    catch { return false; }
  }, async rota => {
    if (rota.request().method() !== 'GET') { await rota.continue(); return; }
    const resposta = await rota.fetch();
    let corpo = await resposta.text();
    if (patchCorpo) corpo = patchCorpo(corpo);
    const cabecalhos = { ...resposta.headers() };
    if (patchCsp) {
      const chave = Object.keys(cabecalhos).find(k => k.toLowerCase() === 'content-security-policy');
      if (chave) cabecalhos[chave] = patchCsp(cabecalhos[chave]);
    }
    await rota.fulfill({ response: resposta, body: corpo, headers: cabecalhos });
  });
}

/** Defeito 1 da SS8: tira a origem da Anthropic do `connect-src`, deixando
 *  so `'self'` -- exatamente o texto que a pagina servia antes desta rodada
 *  de integracao. */
export function csp_apenasSelf(valorAtual) {
  return String(valorAtual).replace(
    /connect-src 'self' https:\/\/api\.anthropic\.com;/,
    "connect-src 'self';");
}

/** Defeito 2 da SS8: "criar do plano sem confirmacao". A linha real de
 *  `claude.js` que liga o clique ao `criarDoPlano` continua LIGADA -- o
 *  defeito e' um `.click()` programado por CIMA dela, no mesmo instante em
 *  que a revisao acaba de desenhar o botao. E' a reproducao mais fiel
 *  possivel do bug historico (a tela criava sem esperar o clique da pessoa)
 *  sem reescrever a funcao inteira: uma linha a mais, no lugar exato onde o
 *  clique de verdade nasceria. */
export function corpo_criaSemConfirmar(html) {
  const alvo = 'onde.querySelector("#iaCriar").onclick = () => criarDoPlano(onde, conf, db);';
  if (!html.includes(alvo)) {
    throw new Error('corpo_criaSemConfirmar: o ponto de reposicao nao foi achado -- '
      + 'claude.js mudou de forma, atualize esta funcao');
  }
  return html.replace(alvo,
    alvo + '\n    // DEFEITO REPOSTO (prova dupla da bateria 2): cria sem esperar o clique.\n'
    + '    onde.querySelector("#iaCriar").click();');
}

/** O terceiro par desta rodada: "chave no corpo de um pedido ao PhxSql". A
 *  `montarContexto` chama `api("tabelas", { database: db })` -- o defeito
 *  historico anexava a chave da Anthropic (`_chave`) a esse corpo, que vai
 *  para o SERVIDOR PHXSQL (nao para a Anthropic). A indentacao de 4 espacos
 *  e' o que distingue este ponto do `api("tabelas", ...)` irmao que existe em
 *  `renderizarPlano` (6 espacos, dentro de um `try`) -- os dois tem o mesmo
 *  texto de chamada, e so o bloco em volta os diferencia. */
/** Reposicao alternativa, SEM interceptacao do Playwright, para os casos em
 *  que a pagina patcheada precisa alcancar OUTRO endereco de loopback (o
 *  servidor falso da Anthropic, tambem em 127.0.0.1).
 *
 *  ACHADO MEDIDO NESTA RODADA: `route.fetch()` + `route.fulfill()` no
 *  documento principal faz o Chromium classificar a pagina resultante como
 *  de um "unknown address space" para fins de Private Network Access -- e
 *  dai TODA chamada seguinte para OUTRO endereco de loopback (a falsa da
 *  Anthropic) e recusada por CORS com "Permission was denied for this
 *  request to access the `unknown` address space", mesmo os dois sendo
 *  127.0.0.1. A pagina servida DIRETO (sem passar pelo `route`) nao sofre
 *  isso -- e' assim que Bateria 1 e Bateria 2 (sem patch) sempre
 *  funcionaram. Ver `docs/cognicao/` desta rodada.
 *
 *  A saida: um PROXY REVERSO de verdade, em `http` puro (zero dependencia),
 *  que o navegador acessa por uma conexao DIRETA (nao por interceptacao) --
 *  ele serve `/` e `/index.html` com o corpo PATCHEADO, e repassa tudo o
 *  mais (`/api`, `/saude`, etc.) para o `phxsqld` de verdade, corpo e
 *  cabecalhos inclusive. Do ponto de vista do Chromium isto e' um servidor
 *  de loopback como outro qualquer -- a classificacao de endereco fica
 *  intacta. */
export async function subirCopiaComPatch({ portaOuvir, portaReal, patchCorpo }) {
  const http = await import('node:http');
  const servidor = http.createServer((req, res) => {
    if (req.url === '/' || req.url === '/index.html') {
      const pedido = http.get({ host: '127.0.0.1', port: portaReal, path: req.url }, r => {
        const pedacos = [];
        r.on('data', c => pedacos.push(c));
        r.on('end', () => {
          let corpo = Buffer.concat(pedacos).toString('utf8');
          if (patchCorpo) corpo = patchCorpo(corpo);
          const cabecalhos = { ...r.headers };
          delete cabecalhos['content-length'];
          delete cabecalhos['transfer-encoding'];
          res.writeHead(r.statusCode, cabecalhos);
          res.end(corpo);
        });
      });
      pedido.on('error', e => { res.writeHead(502); res.end(String(e)); });
      return;
    }
    // Proxy transparente do resto -- e' aqui que `/api`, `/saude` etc.
    // continuam batendo no `phxsqld` DE VERDADE, sem reimplementar o
    // protocolo inteiro so' para testar duas linhas do `claude.js`.
    const encaminhado = http.request(
      { host: '127.0.0.1', port: portaReal, path: req.url, method: req.method, headers: req.headers },
      r => { res.writeHead(r.statusCode, r.headers); r.pipe(res); });
    encaminhado.on('error', e => { res.writeHead(502); res.end(String(e)); });
    req.pipe(encaminhado);
  });
  await new Promise((resolve, reject) => {
    servidor.once('error', reject);
    servidor.listen(portaOuvir, '127.0.0.1', resolve);
  });
  return {
    url: `http://127.0.0.1:${portaOuvir}/`,
    derrubar: () => new Promise(r => servidor.close(r)),
  };
}

export function corpo_chaveNoCorpoDoPedido(html) {
  const alvo = '    const t = await api("tabelas", { database: db });\n    const nomes = t.tabelas || [];';
  if (!html.includes(alvo)) {
    throw new Error('corpo_chaveNoCorpoDoPedido: o ponto de reposicao nao foi achado -- '
      + 'claude.js mudou de forma, atualize esta funcao');
  }
  return html.replace(alvo,
    '    const t = await api("tabelas", { database: db, _chave: cfg().chave });\n'
    + '    const nomes = t.tabelas || [];');
}
