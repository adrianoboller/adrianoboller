# Riscos do PhxSql — a fonte da seção de riscos

Esta tabela é a **fonte** da seção *Riscos* de
`docs/status/status-do-projeto.html`, que **não se edita**:
`python3 docs/status/riscos.py` a lê daqui e escreve a seção na página.

Ela **se edita**, como o `docs/STATUS.md` — porque **probabilidade e impacto
são avaliação, não medida**. Quem discorda de uma linha muda a linha, com o
motivo e a data, e roda o gerador. O que **não** se digita aqui é número: onde
um risco tem número, a linha nomeia uma **sonda**, e o número sai do código na
hora de gerar a página, com a data em que foi medido.

## Como se lê uma linha

| coluna | o que é | quem manda |
|---|---|---|
| `#` | o apelido do risco, estável entre rodadas — não se reaproveita | quem registra |
| `risco` | a frase que diz **o que pode dar errado**, não o que falta fazer | quem registra |
| `prob.` | `certa` · `alta` · `média` · `baixa` — **avaliação**, na data da linha | quem registra |
| `impacto` | `alto` · `médio` · `baixo` — **avaliação**, na data da linha | quem registra |
| `dono` | o papel que responde por ele: `dono` (Adriano) ou a letra da pétrea (`A`–`J`) | quem registra |
| `avaliado em` | a data desta avaliação, em ISO. Linha sem data não entra | quem registra |
| `sonda` | a chave da medição que o gerador resolve, ou `—` | o **código** |
| `fonte` | o documento de onde o risco saiu. **Risco sem fonte não entra** | quem registra |

**`certa` quer dizer que já está acontecendo** — não é previsão, é estado. Uma
catraca vermelha hoje não tem probabilidade de ficar vermelha: ela está.

**As sondas são as do gerador, e a lista sai do código** (`SONDAS`, em
`docs/status/riscos.py`). Sonda que não existe **para a página inteira** com o
nome escrito ao lado — chave morta é pior que chave faltando, porque parece
medição e não mede nada.

## A tabela

| # | risco | prob. | impacto | dono | avaliado em | sonda | fonte |
|---|---|---|---|---|---|---|---|
| R1 | A catraca `alcancam-fsync` está **vermelha** (23 seções críticas alcançam `fsync` com a trava global na mão, teto 22) e a decisão de como fechá-la está com o dono desde 08/09. Enquanto isso, a saída mais barata da pressão é subir o teto — que a pétrea G proíbe | certa | alto | dono | 2026-09-16 | catracas-reprovando | `docs/PENDENCIAS.md` #252 · `docs/CATRACAS.md` §13 |
| R2 | A bateria de ponta a ponta **só roda quando alguém a chama**: não há `cron` neste contêiner. Ela passou de 29/08 a 16/09 sem rodar, e nesses dezoito dias a `alcancam-fsync` esteve furada sem ninguém ver | alta | alto | G | 2026-09-16 | — | `docs/PENDENCIAS.md` #252 · `docs/cognicao/cognicao_catraca-que-so-roda-na-bateria-que-ninguem-roda-e-catraca-frouxa_20260916_0655.md` |
| R3 | Onze guardas do catálogo estavam **quebradas** — o trecho que repõe o defeito não existe mais no código, então a guarda não pode nem ser provada contra o defeito que a motivou. Guarda que não se prova é catálogo, não guarda | certa | alto | G | 2026-09-16 | guardas-vermelhas | `docs/PENDENCIAS.md` #263 |
| R4 | A folha de marca afirma **ACID compliant**, e o «I» entregue sem pedir é `READ COMMITTED`. Quem compra pela folha espera o que o motor não dá por padrão — e a folha não é a fonte da verdade | certa | médio | dono | 2026-09-16 | — | `marca/LEIA-ME.md` · `docs/ACID.md` §2.4/§4.4 · `CLAUDE.md` |
| R5 | `SERIALIZABLE` não existe e **não se reivindica**; a leitura repetível existe pela trava e **pedida**. Quem não pede continua em leitura confirmada e pode ver anomalia que MySQL® e MariaDB® não mostrariam no padrão de fábrica deles | média | médio | B | 2026-09-16 | — | `docs/SOMBRA.md` §5b · `docs/ACID.md` §4.5 · `docs/PENDENCIAS.md` #246 |
| R6 | **Sem TLS real no transporte** — a cifra do fio é a Noise-like desta casa. Os três motores maduros convergem em cifrar a conexão, e o meio (uma crate de TLS) bate na pétrea de zero dependências: é choque de pétrea contra convergência, e está com o dono | média | alto | dono | 2026-09-16 | — | `docs/PENDENCIAS.md` #239 · `docs/CIFRA-DO-FIO.md` · `CLAUDE.md` |
| R7 | Há pedidos abertos **parados com gente**, não com engenharia: o léxico dos gates os acha no `PENDENCIAS.md`. Risco de cronograma que nenhuma frente resolve trabalhando mais | certa | médio | dono | 2026-09-16 | gates | `docs/PENDENCIAS.md`, pelo léxico de `docs/pmo/pagina-do-status-do-projeto.py` |
| R8 | Gatilho `AFTER` que grava pela mesma sessão **dentro do `COMMIT`** não chega a gravar. É perda silenciosa: o commit responde bem e o efeito do gatilho não está lá | alta | alto | B | 2026-09-16 | — | `docs/PENDENCIAS.md` #262 |
| R9 | Tomada no meio de um `BULKINSERT` ou de um `reindexar` deixa a tabela **recusando** até alguém reindexar à mão, e o arranque **não diz nada** sobre isso | média | alto | B | 2026-09-16 | — | `docs/PENDENCIAS.md` #255 |
| R10 | Saúde do disco do banco: sem sonda canário, sem tratar `EROFS` nem erro de E/S na hora. Disco cheio ou montado como somente-leitura vira erro tarde, no meio de uma gravação | média | alto | C | 2026-09-16 | — | `docs/PENDENCIAS.md` #249 · `docs/SAUDE-DO-DISCO.md` |
| R11 | Na promoção de uma réplica **atrasada**, os números de sequência que o master emitiu e esta ponta nunca recebeu não estão no `.reg` daqui: o contador continua de um valor menor que o do master morto. Exige decisão de projeto (faixa por nó ou contador durável propagado) | baixa | alto | C | 2026-09-16 | — | `docs/AUTONUMBER.md` · `docs/PENDENCIAS.md` #229 · `crates/phxsql-store/src/table.rs::reconciliar_sequencia` |
| R12 | `cluster.quorum_minimo` é **guardado e não imposto**: hoje o master confirma sem esperar réplica nenhuma. Está escrito no código e na tela — mas campo que parece efeito é o defeito que o `recursos.cache_paginas` já custou | média | alto | B | 2026-09-16 | — | `docs/PENDENCIAS.md` #207 · `crates/phxsql-server/src/config.rs::quorum_minimo` |
| R13 | O lixo que os `examples/` deixam em `/tmp` está **contado, não medido**, e o zelador **não roda por horário** (não há `cron` aqui). O ambiente já chegou a 560 MB livres sem ninguém saber por quê | alta | médio | D | 2026-09-16 | — | `docs/PENDENCIAS.md` #209 · `CLAUDE.md` (papel D) |
| R14 | O código vive numa **branch de outro repositório** (`adrianoboller/adrianoboller`): o repositório próprio `adrianoboller/phxsql` depende de o dono criá-lo. Custódia do trabalho fora do lugar dele | média | alto | dono | 2026-09-16 | — | `docs/PENDENCIAS.md` #18 · `docs/BACKUP.md` |
| R15 | Prova que **falha sob carga sem defeito nenhum** (`panico_dentro_do_atender_devolve_a_vaga_da_porta_de_dados` exige que os três pânicos aconteçam). Teste instável ensina a ignorar vermelho, que é o pior hábito que uma catraca pode criar | alta | médio | F | 2026-09-16 | — | `docs/PENDENCIAS.md` #267 |
| R16 | O PDCA dos gaps parou em **Plan + Check**: as recomendações estão medidas e a fase **Do** não ocorreu. Pesquisa medida que não vira trabalho envelhece igual a número digitado | certa | médio | A | 2026-09-16 | — | `docs/PDCA-GAPS.md` |
| R17 | A **coluna de data/hora de sistema por linha** — o invariante «impossível o filho ter a mesma data do pai» — está **decidida e não implementada**. É mudança de formato, e mudança de formato entra cedo: depois vira migração | média | alto | C | 2026-09-16 | — | `CLAUDE.md` (invariantes acima do voto) · `docs/CORREIO-CAMPOS.md` |
| R18 | Documento de gate **desatualizado**: `docs/BACKLOG-TIPOS.md` ainda declara o P0 (atomicidade do commit pai+filho) **aberto** e o usa para barrar dois motores, enquanto o pedido #189 está fechado e o `conferir_fks_com` já enxerga a mãe da transação. Frente que o ler bloqueia sem motivo | média | médio | H | 2026-09-16 | — | `docs/BACKLOG-TIPOS.md` §«Os três portões» · `docs/PENDENCIAS.md` #189 · `crates/phxsql-store/src/table.rs::MaesEmProgresso` |
| R19 | A **dívida marcada no fonte não tem catraca**: nada reprova quem acrescentar mais uma. A contagem sobe sozinha e ninguém é obrigado a baixá-la — e catraca é justamente o que impede isso | média | médio | G | 2026-09-16 | divida | `docs/status/riscos.py` (a varredura) · `docs/CATRACAS.md` |

## O que esta tabela NÃO é

**Não é a lista do que falta** — essa é o `docs/PENDENCIAS.md`, e quase toda
linha daqui aponta para um pedido de lá. A diferença é o tempo verbal: a
pendência diz *o que ainda não foi feito*; o risco diz *o que acontece se
continuar assim*. Um pedido aberto que não machuca ninguém não é risco; um
risco pode existir sem pedido nenhum (R17 é um).

**Não é o catálogo de guardas.** Guarda é prova contra um defeito conhecido;
risco é o que ainda não tem prova.

E a regra que a lei da casa impõe aqui: **risco sem fonte não entra.** Risco
inventado numa rodada vira, três rodadas depois, uma preocupação que ninguém
consegue confirmar nem enterrar — e o custo dela é o mesmo de um número
digitado à mão.
