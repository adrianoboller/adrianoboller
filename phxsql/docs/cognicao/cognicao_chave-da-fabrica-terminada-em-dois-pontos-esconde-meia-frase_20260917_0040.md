# Chave da fábrica terminada em «: » esconde meia frase cravada — e nenhuma via do conferidor vê

Hora da descoberta: entre 00:32 e 00:44 de 17/09/2026 — bracketada pelos
`mtime` do `tudo.txt` (a lista do conferidor) e do `bloco_idiomas.rs` (a
primeira escrita do achado) no scratchpad, não pela hora do commit.

## 1. O que aconteceu

Ao levar a tela de Jobs inteira para a fábrica (`crates/phxsql-server/ui/index.html`,
`telaJobs`/`editarJob`), duas chaves que **já estavam na fábrica** e que o
placar contava como cobertas eram meia frase:

```js
avisar(txt("tela.g_ligado_relogio_jobs_nao_subiu","ligado — mas o relógio de jobs não subiu neste arranque: ")
     + "reinicie para ele andar sozinho, ou rode pela tela.", true);
```

O texto da chave terminava em «: », e a segunda metade vinha concatenada
por `+` em português cravado. Em alemão a pessoa lia
«aktiviert — aber die Job-Uhr ist bei diesem Start nicht hochgefahren:
reinicie para ele andar sozinho, ou rode pela tela.» — meia frase em cada
língua, e o placar dizendo que aquela mensagem passava pela fábrica.

Na tela de Jobs eram duas (`tela.g_ligado_relogio_jobs_nao_subiu` e
`tela.g_gravado_relogio_jobs_nao_subiu`). A medição da seção 3 achou a
terceira fora da tela: `tela.st_telemetria_o_que_faz`, terminada em «, ».

## 2. O que eu concluí primeiro, e estava errado

Que a tela de Jobs tinha exatamente os **64** textos que o conferidor
listava, e que traduzir os 64 zerava a tela. Confiei na conta porque a conta
é o que dirige a leva — e a conta estava certa sobre o que ela mede, e cega
para o resto: além das duas metades concatenadas, a tela tinha os ternários
dentro de `${…}` («Ligar»/«Desligar», «Gravar alterações»/«Criar o job»,
«Rodar agora»), o subtítulo do `folha(` depois de um primeiro argumento
`txt(` (a via só olha o primeiro literal, e se ele não é literal ela para),
e os seis rótulos do `JOBS_MODELO` (array de arrays, forma sem receita).
Traduzir «os 64» teria entregado meia tela com o número dizendo zero.

## 3. O que a medição disse

- O conferidor contava **64** em Jobs e **35** em Serviço; a leva baixou o
  placar de **1.049 para 950** — exatamente 99, nem um a mais, apesar de a
  fábrica ter ganhado **101** chaves. As duas a mais são as que a conta não
  via e que deixariam meia tela em português.
- As duas metades concatenadas não aparecem em nenhuma das duas vias por
  construção: a via de rótulo lê o primeiro literal depois de `avisar(`, que
  aqui começa com `txt(` (vira `BURACO`, não é literal, `continue`); a via de
  marcação não olha dentro de JavaScript. O `+ "…"` fica invisível **em
  qualquer posição** — não é falso negativo de aspa nem de crase, é a forma.
- Medido no `index.html` do `HEAD` com uma expressão regular (`txt("tela.…",
  "…")` seguido de `+` e aspa): **5** casos. **3** eram meia frase (os dois
  de Jobs e a descrição da Telemetria, `tela.st_telemetria_o_que_faz`) e
  **2** são legítimos — o segundo termo é separador + **dado**
  (`tela.cl_propagacao + " " + lista`, `tela.confirmar_remover + "\n\n" +
  apelido`). Eu tinha escrito «2 ocorrências, as duas nesta tela» antes de
  medir, e estava errado nos dois números: era o palpite de quem só olhou a
  tela que estava traduzindo.
- Prova real nos dois sentidos com um defeito reposto (`Novo job…` cravado
  de volta): a catraca acusou «951 textos de tela fora da fabrica, e a
  catraca esta em 950» e o laço acusou «tela.jb_novo esta na fabrica e
  nenhuma tela o pede». Restaurado, 1.106 testes do servidor verdes.

## 4. A regra

**Chave da fábrica cujo texto termina em «: », «, », «—» ou espaço é suspeita de
meia frase: procure o `+` logo depois do `txt(` e leve a frase inteira para
a chave.** E ao fechar um lote «tela inteira», leia a tela, não a lista — a
lista diz o que o crivo vê; a tela diz o que a pessoa vê.

## 5. Como está guardado hoje

- As três chaves passaram a carregar a frase inteira nos seis idiomas
  (`idiomas.rs`, comentário ao lado de cada uma: «era MEIA frase»).
- A forma está declarada em `docs/MENSAGENS.md`, seção «O que ainda escapa
  da conta, e por quê», como a terceira forma invisível (ao lado do primeiro
  argumento de `linha(` e do `\uXXXX`).
- A prova pelo navegador (`testes-web/prova-idiomas-jobs-servico.mjs`) liga
  um job com o relógio parado em italiano e exige a **segunda metade** do
  aviso em italiano — é o passo que reprova se alguém voltar a concatenar.
- **Onde o buraco ficou:** o conferidor continua sem ver a forma. Uma
  receita «`txt(` seguido de `+` e literal» pegaria estas três, mas o mesmo
  desenho existe legitimamente quando o segundo termo é dado
  (`txt(…) + esc(nome)`), e distinguir os dois sem falso positivo pede um
  crivo que ainda não foi escrito. Uma receita cega pegaria os 5 e reprovaria
  os 2 certos — guarda que reprova o correto é desligada na primeira semana.
  Três casos é pouco para pagar uma via nova hoje; é demais para fingir que a
  conta os cobre, e por isso a forma está declarada no `MENSAGENS.md`.
