#!/usr/bin/env node
/* PROVA REAL dos casos de botao do pedido 190, nos DOIS sentidos.
 *
 *     node phxsql/testes-web/prova-real-botoes.mjs [--so <pedaco>]
 *
 * Teste que passa por engano e pior que teste que falta. Para cada caso novo,
 * esta prova SERVE a pagina com UM defeito reposto -- o botao morto, o
 * confirm ignorado, a chave inteira no painel -- e exige que o caso REPROVE;
 * e serve a pagina sem defeito e exige que ele PASSE (o controle: um portao
 * que recusasse tudo tambem "reprovaria" o defeito).
 *
 * Sem recompilar o `phxsqld`: a pagina vem de `include_str!`, entao o defeito
 * e reposto num proxy reverso (`subirCopiaComPatch`, o mesmo precedente da
 * `claude-interceptar.mjs`) que reescreve so o documento `/` e repassa o resto
 * ao servidor de verdade. Cada reposicao CONFERE que o trecho existe -- se a
 * tela mudar de forma, a prova grita em vez de passar sem repor nada.
 *
 * Sai com 0 se todos os defeitos foram pegos e todos os controles passaram.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { subirCopiaComPatch } from './claude-interceptar.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');
const PORTA = 6230;                                   // real: +0/+1, proxy: +2
const so = process.argv.includes('--so') ? process.argv[process.argv.indexOf('--so') + 1] : null;

/** Troca EXATAMENTE uma ocorrencia, ou grita. */
const trocar = (de, para) => html => {
  const n = html.split(de).length - 1;
  if (n !== 1) throw new Error(`reposicao: esperava 1 ocorrencia de ${JSON.stringify(de.slice(0, 70))}, achei ${n}`);
  return html.replace(de, () => para);
};

// ------------------------------------------------------------ os defeitos
const DEFEITOS = [
  { caso: 'botoes-de-nova-tabela', nome: 'o «+ campo» do cartao nao faz nada',
    patch: trocar('c.corpo.querySelector("#ntMais").onclick = () => {', 'c.corpo.querySelector("#ntMais").onclick = () => { return;') },
  { caso: 'botoes-de-nova-tabela', nome: 'o «Cadastro completo…» leva um rascunho sem `indices_texto` (o defeito achado clicando)',
    patch: trocar('        indices_texto: [],\n      };\n      c.fechar();', '      };\n      c.fechar();') },
  { caso: 'botoes-de-nova-tabela', nome: 'o «+ indice» da tela cheia nao faz nada',
    patch: trocar('$("#nt_addIdx").onclick = () => {', '$("#nt_addIdx").onclick = () => { return;') },
  { caso: 'botoes-de-nova-tabela', nome: 'o «Voltar» da tela cheia nao volta',
    patch: trocar('$("#nt_voltar").onclick = ev => { ev.preventDefault(); gerirTabelas(db); };', '$("#nt_voltar").onclick = ev => { ev.preventDefault(); };') },
  { caso: 'botoes-do-job', nome: 'o «Rodar agora» da ficha nao roda',
    patch: trocar('$("#btJbRodar").onclick = () => rodarJob(x.nome);', '$("#btJbRodar").onclick = () => {};') },
  { caso: 'botoes-do-job', nome: 'o «Excluir job» ignora o confirm e apaga assim mesmo',
    patch: html => trocar('      if (!confirm(preencher(txt("tela.g_confirmar_excluir_job","Excluir o job {nome}? O histórico das corridas dele fica."), {nome: x.nome}))) return;',
                          '      confirm("x");')(html) },
  { caso: 'botoes-da-telemetria', nome: 'o «Atualizar agora» nao atualiza',
    patch: trocar('$("#tlmAgora").onclick = () => volta();', '$("#tlmAgora").onclick = () => {};') },
  { caso: 'botoes-da-telemetria', nome: 'a pausa nao para o relogio',
    patch: trocar('setInterval(() => { if (!estado.pausado) volta(); }, estado.periodo)', 'setInterval(() => { volta(); }, estado.periodo)') },
  { caso: 'botoes-da-telemetria', nome: 'o «ocultar legenda» nao oculta',
    patch: trocar('estado.legendaAberta = !estado.legendaAberta;', '') },
  { caso: 'botoes-da-claude', nome: 'o painel «o que vai subir» mostra a CHAVE INTEIRA',
    patch: trocar('cab["x-api-key"] = fim(c.chave) ||', 'cab["x-api-key"] = c.chave ||') },
  { caso: 'botoes-da-claude', nome: 'o «Desfazer esta rodada» nao desfaz',
    patch: trocar('onde.querySelector("#iaDesfazer").onclick = () => desfazer(onde, db);', 'onde.querySelector("#iaDesfazer").onclick = () => {};') },
  { caso: 'botoes-da-claude', nome: 'o «Diagrama ER em tela cheia» nao abre o diagrama',
    patch: trocar('onde.querySelector("#iaVerEr").onclick = () => telaDiagramaER(db);', 'onde.querySelector("#iaVerEr").onclick = () => {};') },
  // Pedido 636: a pintura tardia. Cada defeito tira UMA conferencia de posse.
  { caso: 'pintura-tardia', nome: 'o «Criar» do cartao repinta o diagrama sem conferir a posse (o defeito do pedido)',
    patch: trocar('        await montarArvore(false);\n        telaDiagramaER(db, vez);', '        await montarArvore(false);\n        telaDiagramaER(db, vezDoPainel());') },
  { caso: 'pintura-tardia', nome: 'o diagrama pinta o corpo final sem conferir a posse',
    patch: trocar('  if (!aindaNoPainel(minha)) return;\n  ER.db = db;', '  ER.db = db;') },
  { caso: 'pintura-tardia', nome: 'verSequencias (duas fases) pinta sem conferir',
    patch: trocar('  if (!aindaNoPainel(vez)) return;\n  folha(preencher(txt("tela.tt_sequencias_de"', '  folha(preencher(txt("tela.tt_sequencias_de"') },
  { caso: 'pintura-tardia', nome: 'verSessoes (duas fases) pinta sem conferir',
    patch: trocar('  if (!aindaNoPainel(vez)) return;\n  folha(txt("tela.se_titulo", "Sessões"), txt("tela.se_sub"', '  folha(txt("tela.se_titulo", "Sessões"), txt("tela.se_sub"') },
  { caso: 'pintura-tardia', nome: 'telaDbLink (duas fases, ramo «nenhuma ligacao») pinta sem conferir',
    patch: trocar('    if (!aindaNoPainel(vez)) return;\n    folha("DbLink", txt("tela.st_nenhuma_ligacao"', '    folha("DbLink", txt("tela.st_nenhuma_ligacao"') },
  { caso: 'pintura-tardia', nome: 'a grade de conteudo (escreve direto no painel) nao confere a posse',
    patch: trocar('  if (!aindaNoPainel(vez)) return;\n  est.esquemaAtual = e;', '  est.esquemaAtual = e;') },
  { caso: 'pintura-tardia', nome: 'verQuemSou (uma fase) pinta sem conferir',
    patch: trocar('  const q = await api("quem_sou");\n  if (!aindaNoPainel(vez)) return;', '  const q = await api("quem_sou");') },
  { caso: 'pintura-tardia', nome: 'telaMensagens (uma fase, direto no painel) pinta sem conferir',
    patch: trocar('    const m = await api("mensagens");\n    if (!aindaNoPainel(vez)) return;', '    const m = await api("mensagens");') },
  { caso: 'botoes-da-claude', nome: 'o «Remover a chave» nao remove',
    patch: trocar('$("#iaRemover").onclick = () => {', '$("#iaRemover").onclick = () => { return;') },

  // ------------------------------------------------------------- caso 39
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «← Gestao de X» das partições/configuração não volta',
    patch: trocar('  if (b) b.onclick = () => gerirTabela(db, tab);', '  if (b) b.onclick = () => {};') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «← Tabelas de X» da gestão não volta para a lista',
    patch: trocar('  $("#btVoltarTabs").onclick = () => gerirTabelas(db);', '  $("#btVoltarTabs").onclick = () => {};') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «Estrutura completa» nao abre a estrutura',
    patch: trocar('$("#btVerEstr").onclick = () => { est.aba = "estrutura"; abrirTabela(db, tab); };', '$("#btVerEstr").onclick = () => {};') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «Gravar» da importação não grava',
    patch: trocar('  $("#btImportar").onclick = async () => {\n    const texto = previa();', '  $("#btImportar").onclick = async () => {\n    return;\n    const texto = previa();') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «Reparar índice» ignora o confirm e refaz assim mesmo',
    patch: trocar('  if (!confirm(preencher(txt("tela.g_confirmar_reparar_indice","Reparar o índice de {tab}?\\n\\nO .ndx é jogado fora e refeito do zero a partir do .reg."), {tab}))) return;',
                  '  confirm("x");') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «Colar» não cola',
    patch: trocar('  $("#btColarAgora").onclick = async ev => {\n    ev.preventDefault();', '  $("#btColarAgora").onclick = async ev => {\n    ev.preventDefault(); return;') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'o «restaurar» da ficha marcada não restaura',
    patch: trocar('  if (marcada) $("#btRestaurarFicha").onclick = ev => {\n    ev.preventDefault();', '  if (marcada) $("#btRestaurarFicha").onclick = ev => {\n    ev.preventDefault(); return;') },
  { caso: 'botoes-da-gestao-de-tabela', nome: 'a caixa da carga volta a ficar FORA do .form-dbl (o defeito de CSS achado exercitando)',
    patch: html => trocar('reaproveita slot e desfazer deixaria buracos.</span></label>\n     <!-- DENTRO do .form-dbl',
                          'reaproveita slot e desfazer deixaria buracos.</span></label>\n     </div>\n     <!-- DENTRO do .form-dbl')(
                   trocar('Coluna que a tabela não tem é erro; coluna que falta fica nula.</span></label>\n     </div>',
                          'Coluna que a tabela não tem é erro; coluna que falta fica nula.</span></label>')(html)) },

  // ------------------------------------------------------------- caso 40
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Encerrar» da sessão não fecha o soquete',
    patch: trocar('      const r = await api("encerrar_sessao", { id: Number(id), tipo: "conexao" });', '      const r = { encerrada: id, estava: "", aviso: "" };') },
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Encerrar» da sessão ignora o confirm',
    patch: trocar('    if (!confirm(marcado(txt("tela.se_confirma_kill",\n        "Encerrar a conexão {id}?\\n\\nO cliente do outro lado perde a conexão."), { id }))) return;', '    confirm("x");') },
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Encerrar» da sessão web não encerra',
    patch: trocar('        const r = await api("encerrar_sessao", { id, tipo: "web" });', '        const r = { encerrada: id, aviso: "" };') },
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Parar a porta de dados» não para',
    patch: trocar('      const r = await api("servico_parar");', '      const r = { conexoes_abertas: 0 };') },
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Limpar» do profiler não limpa o anel',
    patch: trocar('    await api("profiler_limpar"); profVisto = 0; profLinhas.length = 0;', '    profVisto = 0;') },

  // ------------------------------------------------------------- caso 41
  { caso: 'botoes-da-telemetria-viva', nome: 'o «Derrubar a conexão» não derruba',
    patch: trocar('        const r = await estado.api("encerrar_sessao", { id: a.ligacao, tipo: "conexao" });', '        const r = {};') },
  { caso: 'botoes-da-telemetria-viva', nome: 'o «Derrubar a conexão» ignora o confirm',
    patch: trocar('      if (!confirm(preencher(txt("tela.tl_derrubar_pergunta",\n        "Derrubar a conexão {n}? O soquete fecha e o cliente perde a resposta."),\n        { n: a.ligacao }))) return;', '      confirm("x");') },
  { caso: 'botoes-da-telemetria-viva', nome: 'o «Encerrar a operação» não manda o pedido',
    patch: trocar('        const r = await estado.api("telemetria_encerrar", { id: a.id });', '        const r = { estado: "encerrando", aviso: "x" };') },
  { caso: 'botoes-da-telemetria-viva', nome: 'o «Ver as N desta estação» não entra na estação',
    patch: trocar('      estado.vista = "estacoes";\n      estado.estacao = a.ip;\n      desenhar(estado.ultimo || {});\n    };\n  }', '    };\n  }') },
  { caso: 'botoes-da-telemetria-viva', nome: 'o «Por estação» da trilha não troca a vista',
    patch: trocar('      if (b.dataset.nivel === "estacoes") { estado.vista = "estacoes"; estado.estacao = null; }', '') },

  // ------------------------------------------------------------- caso 42
  { caso: 'botoes-de-lgpd-e-bloqueios', nome: 'o «soltar» não solta o IP',
    patch: trocar('          api("desbloquear", { ip: btn.dataset.ip })', '          Promise.resolve()') },
  { caso: 'botoes-de-lgpd-e-bloqueios', nome: 'o «Salvar whitelist» não salva',
    patch: trocar('          await api("whitelist_salvar", { whitelist: linhas });', '') },
  { caso: 'botoes-de-lgpd-e-bloqueios', nome: 'o «Gerar» ignora o formato escolhido',
    patch: trocar('api("bloqueios_exportar", { formato: $("#fmtExport").value })', 'api("bloqueios_exportar", { formato: "texto" })') },
  { caso: 'botoes-de-lgpd-e-bloqueios', nome: 'o «Varrer de novo» não varre',
    patch: trocar('  $("#btLgVer").onclick = () => telaDadosPessoais($("#lgDb").value || null);', '  $("#btLgVer").onclick = () => {};') },
  { caso: 'botoes-de-lgpd-e-bloqueios', nome: 'o «Reler» da trilha não relê',
    patch: trocar('  $("#btLgTrRec").onclick = () => telaTrilhaLgpd(db, tab, $("#lgTipo").value);', '  $("#btLgTrRec").onclick = () => {};') },
  { caso: 'botoes-de-lgpd-e-bloqueios', nome: 'o nome da tabela volta a atropelar a tela seguinte (.then(irAba))',
    patch: trocar('if (bLg) { est.aba = "estrutura"; abrirTabela(bLg.dataset.db, bLg.dataset.tab); }', 'if (bLg) abrirTabela(bLg.dataset.db, bLg.dataset.tab).then(() => irAba("estrutura"));') },

  // ------------------------------------------------------------- caso 43
  { caso: 'botoes-de-configuracao', nome: 'o «Salvar no config.json» não grava',
    patch: trocar('    const r = await api("config_gravar", { campos });', '    const r = { arquivo: "x", exigem_reinicio: [] };') },
  { caso: 'botoes-de-configuracao', nome: 'o «de fábrica» da cor não limpa o campo',
    patch: trocar('    bloco.querySelector(".cf-cor-zero").onclick = () => {\n      escondido.value = "";', '    bloco.querySelector(".cf-cor-zero").onclick = () => {') },
  { caso: 'botoes-de-configuracao', nome: 'o «Descartar as mudanças» não descarta',
    patch: trocar('  $("#cfDescartar").onclick = () => verConfigServidor();', '  $("#cfDescartar").onclick = () => {};') },
  { caso: 'botoes-de-configuracao', nome: 'o «Voltar aos nomes de fábrica» do menu não volta',
    patch: trocar('    est.rotulos = {};\n    gravarRotulos({});', '') },

  // ------------------------------------------------------------- caso 44
  { caso: 'botoes-de-restaurar-backup', nome: 'o «Substituir» nasce liberado, sem digitar o nome',
    patch: trocar('id="btRstPorCima" disabled>', 'id="btRstPorCima">') },
  { caso: 'botoes-de-restaurar-backup', nome: 'o «Restaurar com este nome» não restaura',
    patch: trocar('    $("#btRstNovo").onclick = () =>\n      restaurarAgora(origem, de, $("#rstNome").value.trim(), false);', '    $("#btRstNovo").onclick = () => {};') },
  { caso: 'botoes-de-restaurar-backup', nome: 'o «Substituir» não manda o confirmar (restaura por cima sem a chave do servidor)',
    patch: trocar('...(porCima ? { modo: "por_cima", confirmar: true } : {}),', '...(porCima ? { modo: "por_cima" } : {}),') },

  // ------------------------------------------------------------- caso 45
  { caso: 'botoes-do-modelo-e-do-conflito', nome: 'o «ao excluir» volta a oferecer cascata/anular/nada (o servidor recusa as três)',
    patch: trocar('${selAcao("fkExc", ["restringir"])}', '${selAcao("fkExc")}') },
  { caso: 'botoes-do-modelo-e-do-conflito', nome: 'o cartão volta a dizer «declarada, não imposta» (mentira de tela)',
    // O texto de reserva so vale quando a CHAVE some da fabrica -- por isso a
    // reposicao tambem troca a chave: sem isso o patch era inerte e a prova
    // PASSAVA (achado da primeira corrida desta prova).
    patch: trocar('txt("tela.fk_card_1",\n      "**Declarada e conferida.** A chave', 'txt("tela.fk_card_1_ausente",\n      "**Declarada, não imposta.** A chave') },
  { caso: 'botoes-do-modelo-e-do-conflito', nome: 'o «Declarar a chave» não declara',
    patch: trocar('      await api("declarar_fk", {\n        database: db, tabela: de.tabela, nome: nomeFk,', '      await (x => x)({\n        database: db, tabela: de.tabela, nome: nomeFk,') },
  { caso: 'botoes-do-modelo-e-do-conflito', nome: 'o «Descartar o meu» não fecha o diálogo de conflito',
    patch: trocar('  fundo.querySelector("#btCfNao").onclick = ev => {\n    ev.preventDefault(); fechar();', '  fundo.querySelector("#btCfNao").onclick = ev => {\n    ev.preventDefault();') },
  { caso: 'botoes-do-modelo-e-do-conflito', nome: 'o «Voltar à lista» não fecha o diálogo da linha excluída',
    patch: trocar('    fundo.querySelector("#btCfFecha").onclick = ev => {\n      ev.preventDefault(); fechar();', '    fundo.querySelector("#btCfFecha").onclick = ev => {\n      ev.preventDefault();') },

  // ------------------------------------------------------------- caso 46
  { caso: 'botoes-de-telas-avulsas', nome: 'o desenho de Venn não roda a junção',
    patch: trocar('    $$("#vennes .venn").forEach(x => x.classList.toggle("viva", x === b));\n    rodarJuncao();', '    $$("#vennes .venn").forEach(x => x.classList.toggle("viva", x === b));') },
  { caso: 'botoes-de-telas-avulsas', nome: '«Guardar esta conexão» guarda também a SENHA (nas DUAS camadas: o formulario e o limpador)',
    // A primeira versao deste defeito so mexia na camada do formulario e
    // PASSOU: `limparConexao` monta o objeto campo a campo e descartava a
    // senha sozinho. A defesa em duas camadas e o desenho; a reposicao tem de
    // quebrar as duas, senao prova que a segunda funciona e nao o caso.
    patch: html => trocar('    database: $("#db").value.trim(),\n    quando: new Date().toISOString(),\n  };\n  const lista = conexoes();',
                          '    database: $("#db").value.trim(),\n    senha: $("#s").value,\n    quando: new Date().toISOString(),\n  };\n  const lista = conexoes();')(
                   trocar('    quando:   String(c.quando ?? "").slice(0, 40),\n  };',
                          '    quando:   String(c.quando ?? "").slice(0, 40),\n    senha:    c.senha,\n  };')(html)) },
  // ------------------------------------------------------------- pedido 644
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Encerrar» da sessao web esquece de dizer o tipo (o servidor teria de adivinhar)',
    patch: trocar('api("encerrar_sessao", { id, tipo: "web" })', 'api("encerrar_sessao", { id })') },
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o «Encerrar» da conexao esquece de dizer o tipo',
    patch: trocar('api("encerrar_sessao", { id: Number(id), tipo: "conexao" })', 'api("encerrar_sessao", { id: Number(id) })') },
  // ------------------------------------------------------------- pedido 645
  { caso: 'botoes-de-sessoes-servico-e-profiler', nome: 'o Profiler volta a cravar «porta 5000» no subtitulo',
    patch: trocar('preencher(txt("tela.st_profiler_o_que_chega","o que chega pela porta {porta}, antes de virar dado"), { porta: est.porta || "?" })', '"o que chega pela porta 5000, antes de virar dado"') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o Sobre volta a cravar «5 arquivos por tabela»',
    patch: trocar('<div class="v">${tipos.length ? tipos.length : "—"}</div>', '<div class="v">5</div>') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o subtitulo da estrutura volta a listar as cinco extensoes de cabeca',
    patch: trocar('$("#subtitulo").textContent = `${database} · ${e.arquivos.join(" + ")}`;', '$("#subtitulo").textContent = `${database} · .reg + .ndx + .bin + .memo + .log`;') },
  { caso: 'botoes-de-telas-avulsas', nome: 'a ficha «versão» do Sobre lê o campo errado do ping (o defeito achado clicando)',
    patch: trocar('v = p.phxsql || p.versao || "—";', 'v = p.versao || "—";') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o ajuste do contador grava um número a mais',
    patch: trocar('api("ajustar_sequencia", { database: db, tabela, proxima: n })', 'api("ajustar_sequencia", { database: db, tabela, proxima: n + 1 })') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o «Atualizar» dos jobs não atualiza',
    patch: trocar('  $("#btJobVer").onclick = () => telaJobs();', '  $("#btJobVer").onclick = () => {};') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o «Alinhar as regiões com os monitores» não alinha',
    patch: trocar('    if (bt) bt.onclick = () => alinharComOsMonitores();', '    if (bt) bt.onclick = () => {};') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o «Fechar» do acompanhar replicação não fecha',
    patch: trocar('  fundo.querySelector("#acFim").onclick = fechar;', '  fundo.querySelector("#acFim").onclick = () => {};') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o «Semear» das mensagens não semeia',
    patch: trocar('        const r = await api("mensagens_semear", {});', '        return; const r = {};') },
  { caso: 'botoes-de-telas-avulsas', nome: 'o «?» da barra não abre o Sobre',
    patch: trocar('$("#btAjuda").onclick = () => verSobre();', '$("#btAjuda").onclick = () => {};') },
  { caso: 'botoes-de-telas-avulsas', nome: 'a celula JSON da grade abre o popover escondido',
    patch: trocar('        popover.hidden = false;\n        popoverDe = jbtn;', '        popoverDe = jbtn;') },
];

const CONTROLES = [...new Set(DEFEITOS.map(d => d.caso))];

// ------------------------------------------------------------- a maquina
async function carregarCaso(nome) {
  const dir = join(AQUI, 'casos');
  for (const f of readdirSync(dir).filter(f => f.endsWith('.mjs')).sort()) {
    const mod = await import(join(dir, f));
    if (mod.caso.nome === nome) return mod.caso;
  }
  throw new Error(`nao achei o caso ${nome}`);
}

/** Roda UM caso, num servidor novo, com ou sem defeito. Devolve a falha ou null. */
async function rodarUm(navegador, caso, patch, tema) {
  const phxsqld = join(RAIZ, 'target', 'release', 'phxsqld');
  const servidor = await subir({ phxsqld, portaDados: PORTA, portaWeb: PORTA + 1, log: () => {} });
  const copia = await subirCopiaComPatch({ portaOuvir: PORTA + 2, portaReal: PORTA + 1, patchCorpo: patch });
  const contexto = await navegador.newContext({ viewport: { width: 1600, height: 950 } });
  await contexto.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch { /* privada */ } }, tema);
  await contexto.route(u => /fonts\.(googleapis|gstatic)\.com/.test(typeof u === 'string' ? u : u.href), r => r.abort());
  const page = await contexto.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(e.message || String(e)));
  const ctx = { page, url: copia.url, tema, base: servidor.base, portaDados: PORTA, portaWeb: PORTA + 1,
                capturas: null, notas: [], nomeCaptura: n => `${caso.nome}-${tema}-${n}` };
  let falha = null;
  try {
    await caso.rodar(ctx);
    if (erros.length) throw new Error(`${erros.length} erro(s) de pagina: ${erros[0]}`);
  } catch (e) { falha = e; }
  await contexto.close();
  await copia.derrubar();
  await servidor.derrubar();
  return falha;
}

async function principal() {
  const navegador = await chromium.launch({ headless: true });
  let ruim = 0;
  const diz = (...a) => console.log(...a);
  try {
    for (const nome of CONTROLES) {
      if (so && !nome.includes(so)) continue;
      const caso = await carregarCaso(nome);
      const f = await rodarUm(navegador, caso, h => h, 'escuro');
      diz(`${f ? 'VERMELHO' : 'verde   '}  controle   ${nome}${f ? `\n      ${String(f.message).split('\n')[0]}` : ''}`);
      if (f) ruim++;
    }
    for (const d of DEFEITOS) {
      if (so && !d.caso.includes(so)) continue;
      const caso = await carregarCaso(d.caso);
      const f = await rodarUm(navegador, caso, d.patch, 'escuro');
      const pegou = !!f;
      diz(`${pegou ? 'RED pego ' : 'PASSOU!! '}  ${d.caso}: ${d.nome}${pegou ? `\n      -> ${String(f.message).split('\n')[0].slice(0, 200)}` : ''}`);
      if (!pegou) ruim++;
    }
  } finally {
    await navegador.close();
  }
  diz(ruim ? `\n${ruim} problema(s): defeito que o caso nao pegou, ou controle que nao passou.` : '\nTodos os defeitos pegos e todos os controles verdes.');
  return ruim ? 1 : 0;
}

principal().then(c => process.exit(c)).catch(e => { console.error(e.stack || e.message); process.exit(2); });
