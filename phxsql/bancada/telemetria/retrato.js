/* Retratos inventados: o modulo nao fala com o servidor, entao da para
   exercitar o desenho com 1, 3, 12 e 40 atividades sem carga nenhuma. */
"use strict";

function serie(n, f) {
  const v = [];
  for (let i = 0; i < n; i++) v.push(f(i));
  return v;
}

function amostras(n) {
  return serie(n, i => ({
    quando_ms: Date.now() - (n - i) * 1000,
    executando: 1 + Math.round(2 * Math.abs(Math.sin(i / 9))),
    esperando: Math.round(3 * Math.abs(Math.sin(i / 7))),
    encerrando: 0,
    ociosas: 2,
    espera_ms_s: 120 * Math.abs(Math.sin(i / 11)),
    espera_maior_ms: 400,
    ler_bytes_s: 900000 * Math.abs(Math.sin(i / 13)),
    escrever_bytes_s: 300000 * Math.abs(Math.cos(i / 8)),
    cpu_processo: 60 + 40 * Math.abs(Math.sin(i / 6)),
    cpu_maquina: 30 + 25 * Math.abs(Math.cos(i / 10)),
    leituras_s: 40 * Math.abs(Math.sin(i / 5)),
    escritas_s: 12 * Math.abs(Math.cos(i / 9)),
    erros_s: i % 37 === 0 ? 1 : 0,
    cache_acertos_s: 800 * Math.abs(Math.sin(i / 4)),
    cache_faltas_s: 90 * Math.abs(Math.cos(i / 6)),
  }));
}

const OPS = ["checksum", "varrer", "inserir_lote", "exportar", "consultar",
             "reindexar", "juntar", "backup", "importar", "pivotar"];

function ativ(i, peso, nivel, extra) {
  const op = nivel === "normal" && i % 3 === 0 ? null : OPS[i % OPS.length];
  return Object.assign({
    id: i % 4 === 3 ? "web:" + (0xa1b2c3d4 + i * 7717).toString(16) : "dados:" + (10 + i),
    origem: i % 4 === 3 ? "web" : "dados",
    ligacao: i % 4 === 3 ? null : 10 + i,
    usuario: ["root", "loja", "relatorio", "etl"][i % 4],
    ip: ["10.0.0.20", "10.0.0.31", "192.168.1.7", "10.0.0.20", "172.16.4.2"][i % 5],
    op,
    alvo: op ? "loja.clientes" : null,
    desde: "2026-08-29T12:00:0" + (i % 10) + "Z",
    aberta_s: 120 + i * 17,
    op_desde: op ? "2026-08-29T12:31:0" + (i % 10) + "Z" : null,
    ha_ms: op ? peso + 300 : 0,
    trabalhando_ms: op ? peso : 0,
    esperou_ms: op ? 300 : 0,
    estado: nivel === "encerrando" ? "encerrando"
      : !op ? "ociosa" : (i % 5 === 1 ? "esperando" : "executando"),
    nivel,
    peso_ms: peso,
    passos: peso * 3,
    pedidos: 4 + i,
    fase: op ? "lendo linhas" : null,
    com_trava: nivel === "stress",
    cancelavel: nivel !== "normal",
    tem_ponto: i % 3 !== 2,
    encerrando: nivel === "encerrando",
    encerradas: 0,
    esperando_o_que: nivel === "alto" ? "a trava de dados" : null,
  }, extra || {});
}

const FIOS = [
  { nome:"aceitador-dados", familia:"servico", finalidade:"fica no accept da porta de dados", viva:true, fazendo:"accept", voltas:9, viva_s:3600 },
  { nome:"amostrador", familia:"servico", finalidade:"tira a amostra das series de segundo em segundo", viva:true, fazendo:"amostrando", voltas:3600, viva_s:3600 },
  { nome:"dados-51234", familia:"atendimento", finalidade:"atende uma conexao da porta de dados", viva:true, fazendo:"checksum", voltas:2, viva_s:31 },
];

function retrato(qtd, forma) {
  const at = [];
  for (let i = 0; i < qtd; i++) {
    let peso, nivel;
    if (forma === "dominante") {
      peso = i === 0 ? 27600 : 300 + i * 40;
      nivel = i === 0 ? "stress" : (i % 4 === 1 ? "alto" : "normal");
    } else if (forma === "parelho") {
      peso = 900 + i * 30;
      nivel = "normal";
    } else {
      peso = Math.round(200 * Math.pow(1.35, qtd - i));
      nivel = i === 0 ? "stress" : i < 3 ? "alto" : i === qtd - 1 && qtd > 4 ? "encerrando" : "normal";
    }
    at.push(ativ(i, peso, nivel));
  }
  return {
    ligada: true, amostrador: true, periodo_ms: 1000,
    agora: "2026-08-29T12:34:56Z",
    ultima_amostra: "2026-08-29T12:34:56Z",
    atraso_ms: 420,
    voce: at.length ? at[at.length - 1].id : null,
    stress: forma === "dominante",
    stress_por_que: forma === "dominante" ? "412 ms de cada segundo na fila da trava de dados" : null,
    limiares: { alto_uso_ms: 2000, stress_ms: 5000 },
    totais: { leituras: 91823, escritas: 4410, erros: 3, espera_ms: 61234,
              trava_ms: 220110, encerramentos: 2, threads_vivas: 7 },
    series: amostras(120),
    atividades: at,
    threads: FIOS,
    cache_ndx: { acertos: 88123, faltas: 4110, acerto_percentual: "95.5", paginas_teto: 4096 },
  };
}

const CENAS = [
  ["1 atividade", () => retrato(1, "dominante")],
  ["3 atividades", () => retrato(3, "escada")],
  ["8 atividades", () => retrato(8, "escada")],
  ["12 atividades", () => retrato(12, "escada")],
  ["40 atividades", () => retrato(40, "escada")],
  ["dominante 27,6 s", () => retrato(8, "dominante")],
  ["todas parelhas", () => retrato(9, "parelho")],
  ["vazio", () => retrato(0, "escada")],
];

let cena = 0;
/* A bancada usa `iniciar`, e nao `desenhar` direto: e `iniciar` que liga a
   busca, a trilha, o congelamento pelo ponteiro e o encerrar. Exercitar sem
   ele seria exercitar metade da tela. */
function montar() {
  PhxTelemetria.parar();
  document.getElementById("painel").innerHTML = PhxTelemetria.html();
  PhxTelemetria.iniciar({
    api: async (op) => {
      if (op === "telemetria") return CENAS[cena][1]();
      return { estado: "encerrando", aviso: "marcada (bancada)" };
    },
    aoAvisar: (t) => { window.ultimoAviso = t; },
    periodo: 2000,
  });
  document.querySelectorAll("#barra button").forEach((b, i) =>
    b.classList.toggle("on", i === cena));
}
const barra = document.getElementById("barra");
CENAS.forEach((c, i) => {
  const b = document.createElement("button");
  b.textContent = c[0];
  b.onclick = () => { cena = i; montar(); };
  barra.appendChild(b);
});
const tema = document.createElement("button");
tema.textContent = "tema";
tema.onclick = () => {
  const r = document.documentElement;
  r.dataset.tema = r.dataset.tema === "claro" ? "" : "claro";
};
barra.appendChild(tema);
window.montarCena = i => { cena = i; montar(); };
montar();
