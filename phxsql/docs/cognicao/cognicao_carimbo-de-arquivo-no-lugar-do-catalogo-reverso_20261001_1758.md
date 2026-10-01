# O carimbo do arquivo faz o papel do catálogo reverso sem cobrar de ninguém — e o medidor do diretório recém-criado não o vê

**Estado:** FRUTÍFERO
**Evidência:** `crates/phxsql-store/src/irmas.rs::a_segunda_exclusao_nao_rele_o_esquema_das_irmas`; `crates/phxsql-store/src/irmas.rs::a_chave_declarada_depois_tranca_o_pai`; `crates/phxsql-store/src/irmas.rs::carimbo_recente_nao_e_confiavel_e_velho_e`

## O que aconteceu

Pedido 259, decisão do dono de 17/09/2026: pular a busca reversa da
integridade quando nenhuma tabela aponta para esta, sem formato novo e sem o
catálogo reverso guardado que a pétrea recusa. Medido antes, 01/10/2026: com
30 irmãs sem chave, a busca custava +418,98 µs por exclusão (14,7× o
excluir), 30 `openat` e 60 `read` — reabrindo esquemas que não tinham mudado.

O conserto (`phxsql-store/src/irmas.rs`) lembra as chaves de cada irmã junto
do carimbo `(dispositivo, inode, tamanho, mtime, ctime)` e revalida por um
`statx`: 441,12 → 51,51 µs com 30 irmãs, 30,39 → 21,88 sozinha, com o
diretório parado (`DESEMPENHO.md` §24.7).

## O que eu concluí primeiro, e estava errado

1. **Que a decisão do dono exigia um catálogo reverso.** «A informação já
   está nos esquemas» parecia pedir uma lista de quem aponta para quem,
   mantida a cada `criar_tabela`/`declarar_fk` — exatamente o que a pétrea
   recusou, e que em memória não enxerga o outro processo. Errado: não é
   preciso **manter** nada se a lembrança se **revalida** a cada pergunta
   pelo carimbo que o núcleo já mantém. O custo vai para quem pergunta (um
   `statx` por irmã), e criação e alteração continuam pagando zero.
2. **Que a primeira corrida do medidor já mostraria o ganho.** A primeira
   corrida do `custo-do-excluir` depois do conserto mostrou a tabela sozinha
   **igual** (35,45 contra 31,51) e as 30 irmãs a 187 µs — e o contador novo
   dizia 9,41 esquemas relidos por exclusão, não zero. O medidor monta o
   diretório no mesmo segundo em que mede, e carimbo com menos de 3 s não se
   lembra por desenho. A régua antiga só mede o diretório recém-criado; o do
   servidor em produção é o parado. Faltava a seção, não o ganho.
3. **Que `(inode, tamanho)` bastava como carimbo.** A declaração de chave
   regrava o bloco de esquema **no lugar** — mesmo inode, mesmo tamanho; só
   `mtime`/`ctime` andam. A guarda `carimbo-da-irma-sem-os-tempos` repõe esse
   carimbo e o pai com filha sai.

## O que a medição disse

- Parado, três pares intercalados: 30,39 (29,98–32,08) → 21,88 (21,49–22,96)
  sozinha; 441,12 (414,98–451,79) → 51,51 (48,66–52,69) com 30 irmãs. As
  faixas não se cruzam.
- Recém-criado, sozinha: 31,05 → 30,29, dentro do ruído — o conserto não
  piora o caso em que não ajuda.
- `openat` com 30 irmãs, parado: 34,4–35,0 → 3,8–4,0 por exclusão, e os que
  sobram são todos `/dev/urandom`.

## A regra

Antes de manter um índice derivado de arquivos, pergunte se o carimbo do
núcleo já o invalida por você — e só confie em carimbo mais velho que o tique
do relógio de arquivos.

## Como está guardado hoje

Três testes em `irmas::testes` e três guardas no catálogo
(`busca-reversa-rele-as-irmas-a-cada-exclusao`, `carimbo-da-irma-sem-os-tempos`,
`carimbo-recente-lembrado`). O medidor ganhou a seção «diretório PARADO» e o
contador `irmas::esquemas_lidos_do_disco`. **O buraco que fica:** em NFS com
relógio do servidor atrasado mais que a margem, um carimbo recente pode
parecer velho; o PhxSql não promete NFS, e isso está escrito no topo do
módulo só por esta linha — não há teste que o pegue.
