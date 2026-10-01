# A politica do diario se perdia na pasta do schema, e o `.reason` nao desliza

**Estado:** PENDENTE

## O que aconteceu

Frente dos pedidos 600, 601 e 245/O2a, 01/10/2026. Duas descobertas que o
texto dos pedidos nao trazia:

- **601.** O pedido nomeava so o embutido: `phx_base_abrir` criava a
  `Instancia` com a politica do diario padrao (desligada). Lendo o
  `Database::recuperar_marcas` (`phxsql-store/src/marca.rs`), apareceu o
  irmao: a pasta de cada SCHEMA vira um `Database::no_diretorio`, que nasce
  com o padrao -- no embutido E no servidor. A marca da cascata mora na pasta
  da tabela (`table.rs`, `proximo_id_no_diretorio(&self.diretorio)`), entao
  a tabela de schema completava o COMMIT sem imagem mesmo com o 564 fechado.
- **600 (b).** O pedido mandava medir se o `motivos` (`.reason`) perde
  registro num expurgo entre paginas, como a trilha perdia (487).

## O que eu concluí primeiro, e estava errado

Que o 600 (b) era o mesmo defeito do 487 e precisava do mesmo cursor. As duas
hipoteses escritas antes de medir: (H1) o `.reason` perde a frente como o
`.lgpd`; (H2) o `.reason` so cresce no fim. H1 morreu: nenhum caminho apaga
registro do `.reason` (`MotivoFile::apagar_tudo` sem chamador), e os dois
expurgos que existem (lixeira e trilha) ACRESCENTAM o proprio rastro no fim.

E no 601, que «o embutido» era o alcance: o `recuperar_no_diretorio` do CLI e
o laco do schema chamam o mesmo `completar_as_marcas_de` na mesma ordem -- sao
irmaos, e o nome do pedido nao os nomeava.

## O que a medição disse

- `os_motivos_paginam_por_pular_sem_perder_registro_no_expurgo`: a pagina 2
  pelo `pular` volta exatamente os registros 3 e 4 depois do esvaziar da
  lixeira e do expurgo da trilha; os expurgos so acrescentam no fim.
- `a_recuperacao_da_base_que_replica_grava_com_imagem_no_schema`: com o
  `no_diretorio` de volta no laco do schema, cai sozinho (guarda
  `recuperacao-do-schema-sem-politica`); o da raiz segue verde.

## A regra

Quando uma politica e herdada por construtor, procure todo `Database` que nasce
por outro construtor dentro do mesmo laco -- e quem recebe so um diretorio tem
de receber a politica como parametro, nunca como padrao.

## Como está guardado hoje

`recuperar_no_diretorio(dir, politica)` exige a politica (o CLI a pede por
`--imagem-no-diario`); o laco do schema usa `no_diretorio_com_politica`.
Guardas `recuperacao-do-embutido-sem-politica` e
`recuperacao-do-schema-sem-politica`. O `.reason` tem a sentinela da premissa,
sem guarda: nao ha conserto para repor.
