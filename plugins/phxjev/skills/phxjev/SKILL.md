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

## 4. Limiares fixos (o veredito sai daqui, nao do juiz)

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

Os limiares sao **aplicados literalmente**. Juiz que ajusta o limiar para caber
no veredito que queria nao julgou — escolheu.

## 5. Formato da saida

```
PhxJev · <preset> · calibracao: nao medida
estado: <n> trechos lidos (<arquivo:linha>, ...)
─────────────────────────────────────────────
<id>  real 0.92  alcancavel 0.88  ja_tratado 0.07  sev 2.4/3 (conf 0.52)  → MANTER ☐ bloqueia
      motivo: <uma linha, com arquivo:linha>
<id>  real 0.31  ...                                                      → DESCARTAR (real<0.50)
─────────────────────────────────────────────
escalar: <lista do que tem conf<0.40, empate ou fere_petrea>
```

## 6. Presets

| Comando | Perguntas |
|---|---|
| `/phxjev-revisar` | por achado: `real`, `alcancavel`, `ja_tratado`, `defeito_ativo` (noul) + `severidade` (score 0–3) |
| `/phxjev-porque` | `causa` (choice sobre ≥2 hipoteses + `outra`) + `proximo_teste` (choice) |
| `/phxjev-escolher` | `melhor_opcao` (choice) + `fere_petrea` (noul por opcao) + convergencia dos motores |
| `/phxjev-perguntar` | pergunta livre, tipo declarado pelo usuario |

## 7. O que falta para deixar de ser palpite

A calibracao se mede, nao se declara: guardar cada veredito do
`/phxjev-revisar` junto do desfecho real (o achado virou conserto? o teste
falhou com o defeito reposto?) e, com ≥50 pares, conferir por faixa se «0,8»
acertou perto de 80%. Ate la, todo aprendizado que nascer de um veredito
PhxJev e **PENDENTE** — veredito nao e evidencia validada e **nunca promove
nada a FRUTIFERO**.
