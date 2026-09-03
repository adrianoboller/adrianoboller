# A afirmação de redundância envelhece pelo MOTIVO antes de envelhecer pelo veredito

**Descoberto em 03/09/2026, 16:50**, auditando as quatro guardas
«REDUNDANTES» do `bancada/guardas/catalogo.py`.

## 1. O que aconteceu

Quatro das 77 entradas do catálogo não provam guarda nenhuma: elas **afirmam**
que tirar aquela metade da proteção não é sentida por teste nenhum, porque a
outra metade cobre sozinha. Saem com veredito `REDUNDANTE` e trazem uma
`nota_da_redundancia` que diz **por que**.

Rodadas hoje, as quatro continuam redundantes:

| guarda | tempo | caíram |
|---|---|---|
| `aad-fora-do-slot` | 8,4 s | 0 de 9 |
| `nonce-sem-endereco` | 9,7 s | 0 de 9 |
| `rest-fecha-sem-escoar` | 14,1 s | 0 de 667 |
| `recuperar-sem-reindexar` | 14,7 s | 0 de 667 |

O veredito está certo. Duas das quatro **notas**, não.

A de `aad-fora-do-slot` diz que a redundância vive «porque o
`nonce_de_pedaco` carrega (rowid, volume, versao)». A de `nonce-sem-endereco`
diz que «o AAD cobre» — e o AAD é `aad_do_slot(volume, rowid, versao)`, os
mesmos três valores.

Só que o teste que decide, `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa`
(`crates/phxsql-store/tests/cifra-dos-dados.rs:404`), copia o slot 5
**inteiro** por cima do slot 9 — cabeçalho junto. A `versao` (offset 8) e o
`tempero` (offset 16) viajam com a cópia, e os dois slots moram no mesmo
volume. **Dos três valores que as notas creditam, dois são idênticos nos dois
lados.** Só o `rowid` difere, e ele difere porque não está gravado em lugar
nenhum: sai da posição em que o slot foi encontrado.

## 2. O que eu concluí primeiro, e estava errado

Que o veredito bastava. A tarefa das quatro entradas parecia ser «rodar e
conferir que ainda dá REDUNDANTE» — e o executor faz isso sozinho, com rigor:
para `espera: "nada muda"` ele exige que **nenhum** teste do binário caia, e
não apenas os da lista.

O que o executor não confere — e ninguém confere — é a **frase**. Uma
redundância pode continuar dando «nada muda» por um motivo diferente do
declarado, e aí a nota vira a única parte do catálogo que envelhece sem
ninguém ser avisado. Foi o que aconteceu: as notas nasceram descrevendo a
*função* (`nonce_de_pedaco` leva três valores) em vez do *teste* (dos três,
só um é diferente entre as duas linhas trocadas).

Também conclui, antes de medir, que o CRC podia estar salvando o teste sem a
cifra entrar na conta — a cópia leva o CRC junto, e um CRC que cobrisse o
endereço reprovaria por outro motivo. Errado: `crc32(&slot[SLOT_CAB..])`
(`reg.rs:1618`) cobre só o corpo, que viaja com a cópia. O CRC bate; quem
recusa é a etiqueta.

## 3. O que a medição disse

Quatro rodadas do binário `cifra-dos-dados` (9 testes, árvore limpa verde),
uma por combinação:

| o que sai | o teste que decide |
|---|---|
| A. só o AAD (a guarda de hoje) | **ok** — 0 de 9 caíram |
| B. o AAD **e** só o `rowid` do nonce (volume e versão ficam) | **FAILED** |
| C. só o endereço do nonce (a guarda de hoje) | **ok** — 0 de 9 caíram |
| D. o endereço do nonce **e** só o `rowid` do AAD (volume e versão ficam) | **FAILED** |

B e D decidem: em cada uma das duas fechaduras, **quem segura sozinho é o
`rowid`**. `volume` e `versao` não contribuem nada para este teste — não por
serem fracos, mas porque são iguais dos dois lados do embaralhamento que ele
faz. Quem tirasse `volume` das duas fechaduras leria as notas como
satisfeitas e estaria certo por acaso; quem tirasse **o `rowid` de uma só**
também leria as notas como satisfeitas — e estaria errado, porque é o único
dos três que faz o trabalho.

## 4. A regra

**Redundância se audita em dois tempos: o veredito e o motivo.** O executor
prova o primeiro; o segundo é uma frase em português que só cai lendo o
código. Quando a nota citar mais de uma coisa como causa, **meça qual delas
sozinha sustenta** — a que não sustenta é a que alguém vai remover confiando
na nota.

## 5. Como está guardado hoje

As notas de `aad-fora-do-slot` e `nonce-sem-endereco` foram reescritas para
nomear o `rowid`, e o comentário do bloco 8 do catálogo passou a dizer por que
`volume` e `versao` não entram na conta **deste** teste. O medidor que produziu
a tabela da seção 3 está em `bancada/guardas/medir-redundancia.py`, para que a
próxima sessão refaça as quatro linhas em vez de acreditar nestas.

**Onde o buraco ficou:** o `julgar` do executor, no ramo `"nada muda"`,
devolve antes de olhar a lista `seguem` — para essas quatro entradas a lista é
decorativa. Não é grave hoje (exigir que *nenhum* teste caia é mais forte que
exigir que os `seguem` passem), mas um `seguem` renomeado ou apagado numa
entrada redundante **não** vira `QUEBRADA`, ao contrário do que aconteceria com
um `caem`. Envelhece calado.

E as outras duas notas, que estão certas pelo motivo declarado, escondem um
**alcance** que vale registrar:

- `rest-fecha-sem-escoar` — a cobertura mora mesmo no passo 13 de
  `bancada/rest/provar.py` (o corpo de 20 000 bytes é calibrado pelo teto de
  `http::escoar`, que o próprio comentário do `http.rs` registra). Mas o
  trecho da guarda é o do `portao_de_rede_http`, que serve REST e Swagger; a
  porta **web** tem a sua própria cópia das mesmas duas linhas
  (`servidor.rs:4195-4196`), e nem a guarda nem o passo 13 passam por ela. O
  `porque` diz «vale para as três portas HTTP»: valem duas.
- `recuperar-sem-reindexar` — a reconstrução mora dentro de `completar()`
  (`transacao.rs:1176`), que só roda no ramo em que a marca confere
  (`completadas += 1`); o outro ramo é `descartadas += 1` e nem chama. O passo
  7 de `bancada/transacoes/provar.py` documenta que o SIGKILL tem **dois
  desfechos legítimos**, e no desfecho ABORTED o defeito é invisível também
  para a bancada. A nota está certa — a cobertura mora lá e em nenhum outro
  lugar —, mas é cobertura que **só dispara quando a corrida cai do lado
  COMMITTED**. Não medi a frequência dos dois desfechos: havia um `phxsqld` de
  outra frente vivo nesta máquina, e o zelador não mata processo dos outros.
