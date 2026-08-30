# Rebuild the config screen
# 29/08 03:03

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
alvo = '''async function verConfig() {
  const c = await api("config");
  folha("Configuração", "o config.json como o servidor o entendeu · o token nunca sai",
        `<pre class="dado">${esc(JSON.stringify(c, null, 1))}</pre>`);
}'''
novo = '''/* O que cada ajuste de `recursos` faz, em uma linha.
 *
 * Fica aqui e não no servidor porque é TEXTO DE TELA: o servidor devolve o
 * valor, e explicar para gente é trabalho da tela. O que não pode é a lista
 * envelhecer calada — por isso o campo que o servidor mandar e que não estiver
 * aqui aparece assim mesmo, com a explicação em branco, em vez de sumir. */
const DIZ_RECURSO = {
  durabilidade:       ["quando o gravado vai de fato para o disco",
                       "por_operacao é seguro e 20× mais lento; por_lote é o padrão"],
  lote_operacoes:     ["gravações por janela de sincronização", ""],
  lote_milissegundos: ["duração máxima da janela", "um relógio de fundo fecha quando ninguém grava"],
  cache_paginas:      ["páginas do .ndx em RAM, por tabela aberta",
                       "4 KiB cada; 2.048 dão 8 MiB. Vale 2,4× na inserção"],
  carga_prazo_min:    ["minutos que um BULKINSERT dura sem ser renovado",
                       "a segunda rede contra reserva órfã — a primeira é a queda da conexão"],
  memoria_max_mb:     ["teto das tabelas residentes (SelectMemory)", "zero = sem teto"],
  threads:            ["linhas de execução do trabalho dividido", "zero = um por núcleo"],
  cpu_percentual:     ["quantos núcleos o trabalho dividido usa", "não é cota do sistema operacional"],
  conexoes_max:       ["soquetes simultâneos aceitos", ""],
  usuarios_max:       ["logins DIFERENTES ao mesmo tempo", "zero = sem teto; é o que uma licença por posto conta"],
};

async function verConfig() {
  const c = await api("config");
  const r = c.recursos || {};

  // A lista de cargas exige administrar. Quem não tem vê a tela sem ela, em
  // vez de ver a tela quebrada.
  let cargas = null;
  try { cargas = await api("cargas"); } catch (e) { /* sem poder: segue */ }

  const linhasRec = Object.entries(r).map(([k, v]) => {
    const diz = DIZ_RECURSO[k] || ["", ""];
    return `<tr>
      <td class="dado"><b>${esc(k)}</b></td>
      <td class="num">${esc(String(v))}</td>
      <td>${esc(diz[0])}${diz[1] ? `<br><span class="leg">${esc(diz[1])}</span>` : ""}</td>
    </tr>`;
  }).join("");

  const linhasCargas = (cargas && cargas.cargas || []).map(x => `<tr>
      <td class="dado"><b>${esc(x.database)}.${esc(x.tabela)}</b></td>
      <td class="dado">${esc(x.usuario || "—")}</td>
      <td class="num">${esc(String(x.ligacao))}</td>
      <td class="dado">${esc(x.desde)}</td>
      <td class="num">${fmt(Math.round(x.expira_em_s))} s</td>
    </tr>`).join("");

  folha("Configuração", "o config.json como o servidor o entendeu · o token nunca sai",
    `<h3 class="secao">Recursos</h3>
     <p class="leg">O que o servidor pode consumir e — o mais importante —
       <b>quando</b> o que foi gravado vai de fato para o disco.</p>
     ${tabela([{t:"campo"},{t:"valor",cls:"num"},{t:"o que faz"}],
              [0], () => linhasRec)}

     <h3 class="secao">Cargas em andamento</h3>
     ${cargas === null
       ? `<div class="nota"><p>A lista de cargas exige <b>administrar</b>.</p></div>`
       : (cargas.total
          ? `${tabela([{t:"tabela"},{t:"usuário"},{t:"ligação",cls:"num"},
                       {t:"desde"},{t:"expira em",cls:"num"}], [0], () => linhasCargas)}
             <p class="leg">Enquanto reservada, a tabela recusa todo mundo com
               <code>EM_CARGA</code> — inclusive a leitura. Solta na queda da
               conexão, ou quando o prazo vence.</p>`
          : `<p class="leg">Nenhuma tabela reservada agora. Um
              <code>BULKINSERT(true)</code> pela porta de dados reserva uma, e
              a carga inteira passa a valer um <code>fsync</code> só.</p>`)}

     <h3 class="secao">O arquivo inteiro, como o servidor o leu</h3>
     <pre class="dado">${esc(JSON.stringify(c, null, 1))}</pre>`);
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
