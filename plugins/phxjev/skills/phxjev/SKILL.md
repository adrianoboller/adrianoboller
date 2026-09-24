---
name: phxjev
description: Juiz tipado do PhxJev. Use quando um passo precisa de DECISAO e nao de texto — triar achado de revisao (real? alcancavel? ja tratado? defeito ativo ou depois da versao?), escolher entre hipoteses de depuracao, escolher entre opcoes de projeto, ou responder uma pergunta sim/nao sobre o codigo. Devolve probabilidade por opcao dentro de um esquema fixo; o veredito sai de limiar fixo, nunca de opiniao.
---

# PhxJev — o juiz tipado

Molde: o **Jev** (TypeSafe), um modelo que nao escreve texto — recebe um
**estado** e um mapa de **perguntas tipadas** e devolve **probabilidade por
opcao**. O PhxJev reproduz o **contrato**, nao o modelo: quem responde e o
proprio agente, seguindo este arquivo ao pe da letra.

## A diferenca que se declara sempre

O Jev e **calibrado**; o PhxJev **nao e**. Toda saida leva a linha
`calibracao: nao medida` ate existir a bancada da secao 7. Probabilidade sem
calibracao e **ordem de grandeza honesta**, nao medida — e o limiar existe
justamente para que ninguem discuta 0,62 contra 0,58.

## 1. Os tres tipos de pergunta

| Tipo | Pergunta | Resposta |
|---|---|---|
| `noul` | «isto e verdade?» | `p` entre 0 e 1 |
| `choice` | «qual rotulo se aplica?» | `p` por rotulo (soma 1) + `conf` |
| `score` | «onde na regua?» | `p` por degrau, posicao = soma(degrau × p) + `conf` |

`conf` resume o **quanto a distribuicao se concentra** (1 − entropia
normalizada). **Nao e acerto.** Serve para uma coisa so: dizer o que escalar.

## 2. O estado

Antes de responder, monte o estado **so com o que foi lido**: trechos com
`arquivo:linha`, saida de comando, numero medido. O que nao foi lido nao entra.
Teto: ~150.000 caracteres, o mesmo do Jev — passou disso, divida em perguntas
menores em vez de resumir de memoria.

## 3. Regras de resposta

1. **Cada probabilidade cita a evidencia** que a moveu (`arquivo:linha` ou o
   comando). Probabilidade sem evidencia fica em **0,5** — nao sei e nao sei.
2. **Conta, data e contagem nao se julgam**: se a pergunta depende de numero,
   rode o comando e ponha o numero no estado. O Jev documenta isso como
   nao-confiavel nele; aqui vale igual.
3. **Hipoteses antes de medir**: `choice` de causa exige no minimo duas opcoes
   escritas antes de abrir qualquer arquivo, e a opcao `outra` sempre existe.
4. **Sem prosa na saida.** O bloco da secao 5 e a resposta inteira; uma linha
   de motivo por pergunta, no maximo.

## 4. Limiares fixos (o veredito sai do script, nao do juiz)

A tabela vive em `scripts/phxjev.py` (constantes `LIMIAR_*`); esta e a copia de
leitura. Mudou la, muda aqui no mesmo commit.

| Pergunta | Regra | Veredito |
|---|---|---|
| `real` | `< 0,50` | **descartar** |
| `alcancavel` | `< 0,30` | **descartar** |
| `ja_tratado` | `> 0,70` | **descartar** (citar onde) |
| `defeito_ativo` | `≥ 0,50` **ou** `conf < 0,40` | **☐ na conta** («na duvida, fica na conta») |
| `defeito_ativo` | `< 0,50` e `conf ≥ 0,40` | **⏸ depois da versao** |
| `fere_petrea` | `≥ 0,30` | **sobe ao dono** — choque com petrea |
| qualquer `choice` | `conf < 0,40` ou top1 − top2 `< 0,15` | **empate** — medir mais; se nada mede, sobe ao dono |
| `severidade` (0–3) | `≥ 2,0` | **bloqueia a entrega** |
| `severidade` | `conf < 0,40` | **escalar: severidade incerta**; se `≥ 2,0`, «bloqueia?» |

Juiz que ajusta o limiar para caber no veredito que queria nao julgou —
escolheu. Por isso quem aplica e o codigo.

## 5. Formato da saida: JSON para o script, nunca bloco escrito a mao

Exercitado ao vivo, o juiz acertou a resposta e **fugiu do formato**
(`conf: alto`, sem a linha da calibracao). Por isso o juiz so devolve
probabilidades; confianca, limiar, veredito e registro saem do script:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" veredito <<'JSON'
{"preset": "revisar",
 "estado": ["reg.rs:17-20", "reg.rs:2026"],
 "itens": [
  {"id": "a1", "motivo": "uma linha",
   "perguntas": {
     "real":          {"tipo": "noul",   "p": 0.92, "evid": "reg.rs:2026"},
     "severidade":    {"tipo": "score",  "p": {"0":0.05,"1":0.15,"2":0.5,"3":0.3}, "evid": "reg.rs:17"},
     "causa":         {"tipo": "choice", "p": {"A":0.7,"B":0.25,"outra":0.05}, "evid": "reg.rs:819"}}}]}
JSON
```

- **Nao escreva `conf`**: o script calcula (1 − entropia normalizada).
- O script **recusa** p sem evidencia (fora de 0,5), choice que nao soma 1 e
  estado vazio. Recusou: corrija o JSON, nao o limiar.
- A resposta ao usuario e a **saida do script, copiada sem editar** — nem
  encurtar caminho. A ultima linha traz um `selo`; `phxjev.py mostrar <selo>`
  reimprime o original, e quem editou fica visivel.
- Sem `CLAUDE_PLUGIN_ROOT` no ambiente, o script mora em
  `plugins/phxjev/scripts/phxjev.py` do repositorio.

## 6. Presets

| Comando | Perguntas |
|---|---|
| `/phxjev-revisar` | por achado: `real`, `alcancavel`, `ja_tratado`, `defeito_ativo` (noul) + `severidade` (score 0–3) |
| `/phxjev-porque` | `causa` (choice sobre ≥2 hipoteses + `outra`) + `proximo_teste` (choice) |
| `/phxjev-escolher` | `melhor_opcao` (choice) + `fere_petrea` (noul por opcao) + convergencia dos motores |
| `/phxjev-perguntar` | pergunta livre, tipo declarado pelo usuario |

## 7. Calibracao: registro, desfecho, medida

Cada veredito entra em `<raiz do git>/.phxjev/registro.jsonl` — **sem
variavel de ambiente**, porque um juiz ja o mandou para o scratchpad dele — com o
desfecho pendente. Quando o desfecho for conhecido — o achado virou conserto?
o teste falhou com o defeito reposto? —:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" desfecho <id> real 1
python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" colher phxsql/docs/PENDENCIAS.md
python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" calibrar   # brier por pergunta + faixas
```

- O registro e **so de acrescimo**: desfecho e linha nova, nunca reescrita.
- `colher` le o fechamento do pedido no git: item `<numero>-...` cujo pedido
  virou ☑️ **depois** do veredito ganha `real=1` e `ja_tratado=0` sozinho.
  Item com letra (`276b`) e pedido ja fechado antes do veredito ficam de fora.
- Um **gancho de fim** recusa encerrar o turno se a resposta nao traz cada
  linha da saida do selo; a segunda tentativa passa.

## 8. Juiz local (Ollama) e a chave `claude | local | auto`

`phxjev.py juiz [claude|local|auto [modelo]]` grava a escolha em
`.phxjev/config.json`. **O pedido nao manda sozinho**: `local` e `auto` so
valem se o modelo passou na bancada (`bancada/resultados.json`: n >= 50,
Brier < 0,10, nenhum erro com confianca > 0,90). Sem isso o comando responde
`juiz claude: auto pedido, mas ...` com o motivo, e o juiz e voce.

Quando o juiz em vigor for local ou auto, voce **nao** da probabilidade:
monta as perguntas e passa ao modelo, com os trechos que leu como contexto.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" auto qwen2.5:3b <<'JSON'
{"estado": ["reg.rs:17-20"], "contexto": ["<texto dos trechos lidos>"],
 "itens": [{"id": "a1", "perguntas": {
   "real": {"tipo": "noul", "pergunta": "O defeito descrito existe no codigo?"},
   "causa": {"tipo": "choice", "pergunta": "Qual a causa?", "opcoes": ["A", "B", "outra"]}}}]}
JSON
```

No `auto`, a linha `ESCALAR AO CLAUDE:` lista o que saiu incerto: essas
perguntas voce julga pela secao 5, como sempre. Nada incerto some.

Para instalar o Ollama: `/phxjev-local-instalar` (pede confirmacao antes).
