# Relatar um volume e enderecar um volume sao caminhos diferentes no `RegFile`

## O que aconteceu

Pedido 222: `esquema.volumes` (a lista que o `esquema` do protocolo devolve
sobre os volumes do `.reg`) vinha **vazia** na particao `PorQuantidade` --
so a `PorPeriodo` a preenchia. `RegFile::fronteiras()`
(`crates/phxsql-store/src/reg.rs`) so' devolvia `&self.fronteiras`, e esse
campo so' nasce nao-vazio quando o modo tem periodo (`RegFile::criar` empurra
a primeira fronteira so' `if r.esquema.paginacao().modo.periodo().is_some()`).
Na por quantidade ele fica `Vec::new()` para sempre, de proposito: e o mesmo
campo que `localizar` e `abrir_faixa_do_periodo` usam para achar o volume de
um rowid por busca binaria, e ali ele precisa mesmo ficar vazio para o codigo
cair no ramo da DIVISAO (`Paginacao::localizar`).

O conserto (frente G3, pedido 222) trocou `fronteiras()` de
`pub fn fronteiras(&self) -> &[Fronteira]` para
`pub fn fronteiras(&self) -> Vec<Fronteira>`, e agora, quando o cache interno
esta vazio e o modo nao e alfanumerico, ele CALCULA a lista a partir de
`self.volumes.existentes()` (quais arquivos `_NNN.reg` existem em disco) e de
`primeiro_rowid_do_volume` (a mesma formula que o enderecamento ja usava).
`self.fronteiras` nunca e escrito por este metodo -- so' lido -- entao o
enderecamento continua tomando exatamente o mesmo caminho de antes.

## O que eu concluí primeiro, e estava errado

O pedido dizia para preencher `volumes` "a partir dos arquivos `_001`...`_NNN`
que ja existem em disco e do `slot_count` de cada cabecalho". Li isso como
"tenho de abrir cada volume e ler o `slot_count` gravado nele" -- e caminhei
para desenhar um `reler_contagens_por_quantidade()` que faria uma leitura de
128 bytes por volume, no mesmo espirito de `reler_baldes`/`reler_fronteiras`.

Medindo o formato antes de escrever (`montar_cabecalho`, offsets 20..28),
descobri que isso estaria errado: `slot_count` e' um contador **da tabela
inteira**, gravado **so' no volume 1** (`if volume == 1 { por_u64(&mut buf,
20, self.slot_count); ... }`). Os outros volumes nunca carregam o proprio
`slot_count` -- carregam zero ali, porque ninguem escreve esse campo neles.
Ler o cabecalho de cada volume para tirar dele uma contagem que nao existe
teria devolvido zero para todo volume que nao fosse o 1, e o defeito teria
saido do jeito mais traicoeiro: sem erro nenhum, so' com o numero errado.

## O que a medição disse

- `slot_count` (offset 20 do cabecalho) so' e' escrito quando `volume == 1`
  (`crates/phxsql-store/src/reg.rs`, `montar_cabecalho`) -- confirmado lendo
  o codigo, nao supondo.
- A conta que falta para "quantas linhas cada volume tem" nao precisa de
  NENHUMA leitura nova de disco: `self.slot_count` (a marca d'agua global,
  ja em memoria) e `self.volumes.existentes()` (quais arquivos existem, um
  `stat` por candidato, ja pago por `Table::volumes_por_arquivo` noutro
  ponto do `esquema`) bastam. O teste
  `esquema_devolve_volumes_com_a_contagem_certa_na_particao_por_quantidade`
  (`crates/phxsql-server/src/servidor.rs`) prova isso com 25 linhas e
  `registros_por_arquivo=10`: volumes `[1, 11, 21]`, contagens `[10, 10, 5]`.
- Reposto o defeito (fazendo `fronteiras()` voltar a so' devolver
  `self.fronteiras.clone()`), o mesmo teste cai na asserção de tamanho:
  `left: 0, right: 3` -- "veio []" em vez dos 3 volumes esperados. O teste da
  particao por periodo, no mesmo arquivo, continua passando com o defeito
  reposto: prova que o conserto e' isolado ao ramo por quantidade.

## A regra

**Um metodo de RELATO pode calcular o que um metodo de ENDERECAMENTO precisa
encontrar pronto, desde que leia os mesmos campos e nunca escreva no cache que
o enderecamento consulta.** Antes de acrescentar uma leitura de disco nova
para "completar" um relato, confira se o dado que falta ja mora em memoria sob
outro nome -- um contador de tabela inteira gravado num unico volume nao vira
contador por volume so' porque o relato precisa de um por volume.

## Como está guardado hoje

- O calculo mora em `RegFile::fronteiras()`
  (`crates/phxsql-store/src/reg.rs`), com o comentario explicando por que e'
  seguro para o enderecamento (a secao "Pedido 222" do proprio doc-comment).
- `Table::fronteiras()` (`crates/phxsql-store/src/table.rs`) e' o
  encaminhador; mudou de `&[Fronteira]` para `Vec<Fronteira>` porque agora
  pode devolver uma lista calculada, nao so' emprestada.
- A prova real (defeito reposto -> cai; conserto -> passa; controle da
  particao por periodo -> continua passando) esta em
  `crates/phxsql-server/src/servidor.rs`, modulo
  `testes_volumes_por_quantidade`.
- `bancada/particao-por-faixa/sonda.py` documentava o campo vazio como "nao e
  defeito" -- corrigido no mesmo commit para nao envelhecer calado; a sonda
  em si so' roda contra o binario compilado e não é a prova real (essa e' o
  teste acima).
