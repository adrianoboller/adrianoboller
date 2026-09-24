// A prova das guardas da tela do PhxZip: cada guarda do `exercitar.mjs` tem de
// FALHAR com o defeito que a motivou reposto, e PASSAR sem ele.
//
//     node testes-web/phxzip/prova-das-guardas.mjs
//
// Teste que passa por engano e pior que teste que falta. Esta casa ja teve uma
// prova que passava com o defeito reposto porque conferia o veredito depois do
// dano. Aqui cada mutacao e aplicada numa COPIA da tela (um diretorio
// temporario), servida pelo servidor falso com `--ui`: o arquivo versionado
// nunca e tocado, e uma corrida interrompida nao deixa defeito para tras.
//
// Os dois sentidos, por guarda: a mesma cadeia de casos roda na copia limpa (tem
// de passar) e na copia com o defeito (tem de falhar, com a mensagem esperada --
// falhar por outro motivo nao prova a guarda, prova um acidente).
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const AQUI = dirname(fileURLToPath(import.meta.url));
const UI = resolve(AQUI, "../../crates/phxzip-web/ui");
const EXERCITAR = join(AQUI, "exercitar.mjs");
const PORTA = 7789;
// `--so <pedaco>`: so as mutacoes cujo nome contem o pedaco.
const SO = (() => { const i = process.argv.indexOf("--so"); return i > 0 ? process.argv[i + 1] : null; })();

const SEIS = texto => JSON.stringify(Object.fromEntries(
  ["Portugues", "Frances", "Ingles", "Italiano", "Alemao", "Espanhol"].map(c => [c, texto])));

// Cada mutacao: o defeito que JA aconteceu (aqui ou na tela do PhxSql), onde
// repo-lo, que caso tem de acusar, e com que frase.
const MUTACOES = [
  {
    nome: "find no lugar de filter (a licao do rownum)",
    guarda: "enter-compacta", cadeia: "compactar-arquivos-e-pasta,soltar,phz,senhas,enter-compacta",
    arquivo: "phxzip.js",
    trocas: [["const carga = est.itens.filter(i => !i.pasta).map(i => i.arquivo);",
      "const carga = [est.itens.find(i => !i.pasta)].map(i => i.arquivo);"]],
    espera: /PEDIDO_MALFORMADO/,
  },
  {
    nome: "dado em caixa alta (o «Blumenau» que virou «BLUMENAU»)",
    guarda: "compactar-arquivos-e-pasta", cadeia: "compactar-arquivos-e-pasta",
    arquivo: "phxzip.css",
    trocas: [["text-transform: none !important;", "text-transform: inherit;"],
      [".grade td { padding: 7px 10px;", ".grade td { text-transform: uppercase; padding: 7px 10px;"]],
    espera: /dado desenhado diferente do gravado: .*Blumenau\.txt -> BLUMENAU\.TXT/,
  },
  {
    nome: "texto solto em flex (o «, die der Server» com espaco antes da virgula)",
    guarda: "compactar-arquivos-e-pasta", cadeia: "compactar-arquivos-e-pasta",
    arquivo: "phxzip.js",
    trocas: [['h("span", null, frase(estourou ? "zip.acima_do_teto" : "zip.dentro_do_teto", { usado: fmtTam(envio), teto: fmtTam(teto) })));',
      'frase(estourou ? "zip.acima_do_teto" : "zip.dentro_do_teto", { usado: fmtTam(envio), teto: fmtTam(teto) }));']],
    espera: /texto solto em flex\/grid: #resumoTeto/,
  },
  {
    nome: "nome de entrada por innerHTML",
    guarda: "nomes-hostis", cadeia: "nomes-hostis",
    arquivo: "phxzip.js",
    trocas: [["if (ultimo < nome.length) s.append(nome.slice(ultimo));",
      'if (ultimo < nome.length) s.insertAdjacentHTML("beforeend", nome.slice(ultimo));']],
    espera: /nao apareceu escrito|<img> nasceu/,
  },
  {
    nome: "senha na URL",
    guarda: "senha-so-no-corpo", cadeia: "compactar-arquivos-e-pasta,soltar,phz,senhas,enter-compacta,senha-so-no-corpo",
    arquivo: "phxzip.js",
    trocas: [['x.open("POST", rota);',
      'x.open("POST", rota + "?senha=" + encodeURIComponent($("#inSenha").value || $("#inSenhaPacote").value));']],
    espera: /senha no log do servidor/,
  },
  {
    nome: "texto cravado no JS",
    guarda: "pseudoidioma", cadeia: "pseudoidioma",
    arquivo: "phxzip.js",
    trocas: [['const selos = h("div", { class: "selos" });',
      'const selos = h("div", { class: "selos" }, h("span", null, "Métodos:"));']],
    espera: /texto fora da fabrica: .*Métodos:/,
  },
  {
    nome: "chave morta no dicionario",
    guarda: "laco-das-chaves", cadeia: "laco-das-chaves",
    arquivo: "textos.json",
    trocas: [['"textos": {', `"textos": {\n  "zip.chave_que_ninguem_pede": ${SEIS("x")},`]],
    espera: /chave morta \(ninguem pede\): zip\.chave_que_ninguem_pede/,
  },
  {
    nome: "chave pedida e ausente",
    guarda: "laco-das-chaves", cadeia: "laco-das-chaves",
    arquivo: "index.html",
    trocas: [['<p class="zona-titulo" data-txt="zip.solte_arquivos"></p>',
      '<p class="zona-titulo" data-txt="zip.solte_arquivos"></p><p data-txt="zip.chave_que_nao_existe"></p>']],
    espera: /chave pedida e ausente: zip\.chave_que_nao_existe/,
  },
  {
    nome: "acao com fundo cheio em repouso",
    guarda: "cores-escuro", cadeia: "cores-escuro",
    arquivo: "phxzip.css",
    trocas: [["background: transparent; color: var(--cor);", "background: var(--cor); color: var(--tinta-botao);"]],
    espera: /fundo cheio em repouso/,
  },
  {
    nome: "cor do tema escuro sobre o papel",
    guarda: "tema-claro", cadeia: "tema-claro",
    arquivo: "phxzip.css",
    trocas: [["--acao-marcar: #b5257f; --acao-consultar: #1f5c93;", "--acao-marcar: #b5257f; --acao-consultar: #5fa6e8;"]],
    espera: /contraste abaixo de 4,5:1/,
  },
  {
    nome: "[hidden] vencido por display",
    guarda: "limpar-leva", cadeia: "compactar-arquivos-e-pasta,limpar-leva",
    arquivo: "phxzip.css",
    trocas: [["[hidden] { display: none !important; }", "[hidden] { display: none; }"]],
    espera: /as opcoes ficaram com a lista vazia/,
  },
  {
    nome: "limpar a lista esquece a senha no campo escondido",
    guarda: "limpar-leva", cadeia: "compactar-arquivos-e-pasta,senhas,limpar-leva",
    arquivo: "phxzip.js",
    trocas: [['$("#inSenha").value = ""; $("#inSenha2").value = ""; $("#inNomePacote").value = "";', '$("#inNomePacote").value = "";']],
    espera: /a senha ficou escondida no campo/,
  },
];

function rodar(dir, cadeia) {
  const res = join(dir, "..", `res-${Date.now()}-${Math.random().toString(36).slice(2)}.json`);
  const r = spawnSync("node", [EXERCITAR, "--ui", dir, "--so", cadeia, "--sem-capturas", "--porta", String(PORTA), "--resultado", res],
    { encoding: "utf8", timeout: 180000 });
  let j = null;
  try { j = JSON.parse(readFileSync(res, "utf8")); } catch { j = null; }
  rmSync(res, { force: true });
  return { j, saida: (r.stdout || "") + (r.stderr || "") };
}

console.log(`Prova das guardas da tela do PhxZip (${new Date().toISOString()})\n`);
const linhas = [];
let ruins = 0;
for (const m of MUTACOES.filter(m => !SO || m.nome.includes(SO))) {
  const base = mkdtempSync(join(tmpdir(), "phxzip-prova-"));
  const limpa = join(base, "limpa");
  const mutada = join(base, "mutada");
  cpSync(UI, limpa, { recursive: true });
  cpSync(UI, mutada, { recursive: true });
  let fonte = readFileSync(join(mutada, m.arquivo), "utf8");
  for (const [de, para] of m.trocas) {
    const n = fonte.split(de).length - 1;
    if (n !== 1) { console.log(`  ERRO  ${m.nome}: o trecho a repor aparece ${n} vez(es) em ${m.arquivo} -- a mutacao envelheceu`); ruins++; fonte = null; break; }
    fonte = fonte.replace(de, para);
  }
  if (fonte === null) { rmSync(base, { recursive: true, force: true }); continue; }
  writeFileSync(join(mutada, m.arquivo), fonte);

  const sem = rodar(limpa, m.cadeia);
  const com = rodar(mutada, m.cadeia);
  rmSync(base, { recursive: true, force: true });
  const caso = (r) => r.j && r.j.resultados.find(x => x.nome.includes(m.guarda));
  const cs = caso(sem), cc = caso(com);
  const passaSem = cs && cs.ok;
  const falhaCom = cc && !cc.ok && m.espera.test(cc.nota);
  const ok = passaSem && falhaCom;
  if (!ok) ruins++;
  const linha = `${ok ? "  ok  " : "  RUIM"}  ${m.nome}\n        sem o defeito: ${cs ? (cs.ok ? "passa" : "FALHA — " + cs.nota) : "caso nao rodou"}\n        com o defeito: ${cc ? (cc.ok ? "PASSA (a guarda nao ve o defeito)" : "falha — " + cc.nota) : "caso nao rodou"}`;
  console.log(linha);
  linhas.push({ mutacao: m.nome, guarda: m.guarda, passaSem: !!passaSem, falhaCom: !!falhaCom, nota: cc ? cc.nota : null });
}
if (!SO) writeFileSync(join(AQUI, "capturas", "prova-das-guardas.json"),
  JSON.stringify({ quando: new Date().toISOString(), guardas: linhas.length, ruins, linhas }, null, 1) + "\n");
console.log(`\n${linhas.length - ruins}/${linhas.length} guardas provadas nos dois sentidos`);
process.exit(ruins ? 1 : 0);
