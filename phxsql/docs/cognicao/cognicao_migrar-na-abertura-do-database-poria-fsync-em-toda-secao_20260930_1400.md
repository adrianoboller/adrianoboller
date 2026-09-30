# Migrar o formato na abertura do DATABASE poria `fsync` em toda seção da trava

**Estado:** PENDENTE

## O que aconteceu

Pedido 508: o separador de volume no nome do arquivo trocou de `_` para `#`, e
o disco do binário anterior tem de ser renomeado uma vez, com `fsync` do
diretório e uma marca durável (`_formato-volumes.json`) por diretório. O lugar
«natural» para disparar a migração era `Instancia::abrir_database`: é por ali
que todo acesso passa, e a migração ficaria preguiçosa e automática.

## O que eu concluí primeiro, e estava errado

Que bastava um cache em memória (`HashSet` dos databases já conferidos) para a
abertura de database ficar barata, e que a migração ali era segura porque «roda
uma vez». As duas metades erravam o que importa:

- **Custo não é só tempo, é o que a seção alcança.** `abrir_database` está em
  quase toda seção crítica do servidor; pendurar nela um caminho até `sync_all`
  faz o `mapa-da-trava.py` contar essas seções como «alcançam `fsync` com a
  trava na mão», mesmo que o ramo só rode uma vez na vida do diretório. A
  catraca `alcancam-fsync-2` não sobe — e ela estaria certa em reprovar: o
  ramo existe no código de todas elas.
- **A ficha compartilhada não escreve.** `Raiz::abrir_para_ler` passa pela
  mesma `abrir_database`; migrar ali seria renomear arquivos debaixo de
  leitores em paralelo.
- **O cache mata a segunda restauração.** O palco da restauração é um caminho
  que se reusa; lembrar dele como «migrado» faria o próximo backup antigo pular
  a migração. O cache só serve à pergunta, nunca à migração.

## O que a medição disse

`mapa-da-trava.py --numeros` depois do desenho final: `alcancam-fsync-2`
medido **23**, igual ao teto. A migração mora na abertura da **raiz**
(`separador::migrar_base`, chamada pela subida do servidor e por
`Instancia::nova`) e no palco da restauração; a abertura do database só confere
a marca, marca sem `fsync` o diretório que não tem nada a renomear, e recusa o
que teria. A ficha compartilhada devolve `PrecisaDaFichaExclusiva`.

## A regra

Trabalho de uma vez só (migração, reparo de formato) mora onde não há trava
nem leitor — a abertura da raiz —, e o caminho quente só **pergunta**; ramo
raro dentro da seção conta como se rodasse sempre.

## Como está guardado hoje

- A catraca `alcancam-fsync-2` do `bancada/concorrencia/mapa-da-trava.py`.
- `tests/separador-de-volume.rs` (a migração, a queda entre dois `rename`s por
  `SIGKILL`, o mesmo volume nos dois nomes) e as guardas
  `migracao-do-separador-decide-pelo-nome` e
  `marca-do-separador-antes-dos-renomes` no `bancada/guardas/catalogo.py`.
- **O buraco:** um diretório velho copiado para dentro da base com o processo
  no ar, sob o nome de um database que já respondeu «migrado» nesta vida do
  processo, só migra na subida seguinte (o cache diz «não» antes do disco).
