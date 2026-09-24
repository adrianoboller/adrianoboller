# A visão do que vai ser gravado sai das funções que gravam — ou diverge para os dois lados

**Estado:** PENDENTE

## 1. O que aconteceu

A revisão adversária do DBA (`docs/propostas/parecer-dba-448-2026-09-24.md`) bloqueou a primeira versão do pedido 448 por três achados ALTOS e um quarto na conta. Os quatro têm a mesma forma: uma resposta que o motor dá em dois lugares, calculada de dois jeitos.

- **A1:** a pré-conferência planejava a cascata com o prefixo, e a passada planejava de novo abrindo a filha por um segundo descritor. `[inserir pedido→A, alterar a chave de B]`, com B sem filha, passava na primeira e quebrava na segunda: «DEFEITO DO MOTOR».
- **A2:** o plano abria a filha com uma **cópia** da sobreposição dela, uma cópia por alteração de chave.
- **A3:** a sobreposição guardava a linha **crua** do `empilhar`, e o store grava a **completada** (DEFAULT, `Sequence`, calculada, colunas de sistema).
- **A4:** a regra do NULL no índice único estava escrita no store e no servidor, e as duas divergiam.

## 2. O que eu concluí primeiro, e estava errado

- **Que a passada, virando cinto, ficava inofensiva.** Não ficava: o cinto também **planejava**, e dois planejadores são duas respostas. O que eu tinha medido era só «a pré-conferência aprova o que deve». Não medi que a passada **aplica** o que a pré-conferência aprovou.
- **Que a cópia da sobreposição no plano era custo de caminho raro.** Escrevi isso no raciocínio («alteração de chave com prefixo enorme é raro») e não medi. A sonda do DBA mediu 715 ms → 99,6 s com n = 8.000. Na minha máquina, com o binário da primeira versão, deu 92.618,7 ms.
- **Que a linha crua na sobreposição era uma imprecisão só da leitura.** Na conferência ela recusava lista válida (`[mãe pelo DEFAULT 7, filha→7]`). No `empilhar`, fazia a `Sequence` não mandada parecer chave que virou NULO, e a cascata levava o NULO às filhas.

## 3. O que a medição disse

- **A1:** as duas provas do achado caem com os elos fora da lista, e passam com eles dentro.
- **A2** (`custo-da-pre-conferencia chaves`, n = 2.000 / 4.000 / 8.000):

| | n = 2.000 | n = 4.000 | n = 8.000 |
|---|---|---|---|
| antes do 448 (`82a17ef`) | 207,0 ms | 375,1 ms | 740,4 ms |
| com a cópia | 3.583,8 ms | 15.181,2 ms | 92.618,7 ms |
| com o `Arc` | 628,7 ms | 1.345,2 ms | 2.763,1 ms |

  Com o `Arc`, o custo dobra a cada dobra de n (×2,14, ×2,05), mas fica 3,0× a 3,7× acima de antes do 448. A prova no teste conta as cópias: eram 5, uma por alteração; passaram a 0.
- **A3:** quatro provas vermelhas na primeira versão, verdes com a previsão pelas funções da gravação.
- **A4:** com um NULL no disco, `[id=3 email=x, id=4 email=NULL]` saía com 1 gravada e «DEFEITO DO MOTOR»; agora saem as 2.

## 4. A regra

Toda visão do que vai ser gravado — sobreposição, plano, previsão — se calcula pelas mesmas funções que vão gravar, e uma decisão só mora num lugar. A cópia que «só lê» se divide; não se copia.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/servidor.rs::a1_a_mae_sem_filha_muda_de_chave_depois_de_a_lista_escrever_na_filha`, com os irmãos do módulo `revisao_do_dba_448` (A1 a A4).
- `crates/phxsql-store/tests/nulo-no-indice-unico.rs::varios_nulos_num_indice_unico_passam_por_todas_as_portas`.
- As guardas do catálogo `passada-replaneja-a-cascata`, `prefixo-copia-a-sobreposicao`, `sobreposicao-guarda-a-linha-crua`, `nulo-colide-no-unico` e `nulo-colide-no-unico-do-commit`.
- **O buraco que fica:** a constante do A2. Cada alteração de chave replaneja abrindo a filha e lendo o esquema das irmãs, uns 345 µs cada, contra 93 µs antes do 448. É a mesma família do pedido 494 (cada conferência reabre a mãe), e ele está ⏸.
- **O `rowstamp` e o `rowtime` da linha nascida não se preveem.** O carimbo é um contador do processo, e o relógio é o da gravação. Ninguém os referencia por chave hoje, e isso está escrito na `Previsao`.
