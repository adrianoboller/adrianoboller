# Bloco de `if` repetido, sem nome, não avisa quando um campo novo precisa se cadastrar — pedido 225

## 1. O que aconteceu

Pedido 225: `jobs.json` e `acessos.log` apareceram na raiz do repositório
depois de uma bancada subir um `phxsqld` de lá. Antes de escrever qualquer
correção, fui ler `Config::ler` para entender como os caminhos eram lidos —
e achei que a resolução "caminho relativo mora ao lado do `config.json`" **já
existia**, desde o primeiro commit do servidor (27/08/2026):

```rust
if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
    if c.base.is_relative() { c.base = dir.join(&c.base); }
    if c.log_acessos.is_relative() { c.log_acessos = dir.join(&c.log_acessos); }
    if c.blacklist.is_relative() { c.blacklist = dir.join(&c.blacklist); }
    if c.backup.destino.is_relative() { c.backup.destino = dir.join(&c.backup.destino); }
}
```

`dblink` e `jobs` — os dois campos que o pedido citava — **não estavam nesta
lista**. Os dois entraram no `Config` bem depois (a seção de `jobs.rs` e
`dblink.rs` é de rodadas posteriores), e ninguém voltou a este bloco para
acrescentá-los.

## 2. O que eu concluí primeiro, e estava errado

Antes de ler o código, concluí — pelo texto do `PENDENCIAS.md`, que descrevia
o defeito como "`log_acessos`/`blacklist.json`/`jobs.json` todos relativos ao
cwd do processo" — que os **três** estavam igualmente quebrados, e que o
conserto era escrever a resolução do zero para os três. Estava errado: dois
terços do defeito relatado (`log_acessos`, `blacklist`) já tinham conserto
funcionando havia dias; só `jobs` (o citado) e `dblink` (que o `PENDENCIAS`
não citava, mas tem o mesmo formato) estavam de fora. Reescrever os três do
zero teria arriscado duplicar ou divergir da lógica que `base`/`log_acessos`/
`blacklist`/`backup.destino` já tinham, testada, havia dias.

## 3. O que a medição disse

Um teste novo (`caminhos_padrao_resolvem_contra_o_diretorio_do_config`),
carregando um `config.json` mínimo com `Config::ler` de dentro de um
diretório temporário, confirmou exatamente o corte: `base`, `log_acessos`,
`blacklist` e `backup.destino` já resolviam certo (o teste passava para os
quatro mesmo antes de eu tocar no código); `dblink` e `jobs` vinham como
`"dblink.json"`/`"jobs.json"` **crus**, sem o diretório do config na frente.
Reposto o defeito (as duas linhas de `dblink`/`jobs` removidas do bloco), o
teste falha nomeando `dblink` — e o teste de integração pelo processo real
(`caminhos-do-config.rs`, subindo o `phxsqld` de um diretório e o config
noutro) falha dizendo que `jobs.json` nasceu no diretório errado, reproduzindo
o sintoma original ponto a ponto.

## 4. A regra

Um bloco de `if campo.is_relative() { ... }` repetido, sem um nome que diga
"aqui é onde todo caminho do config se resolve", não é um lugar que um campo
novo sabe que precisa procurar. Diferente de uma lista/array nomeada (tipo
`EXTENSOES_TODAS`, onde falta um item é visualmente óbvio ao ler a lista), um
bloco de quatro `if`s soltos não tem CONTORNO: quem acrescenta o quinto campo
de caminho ao `Config` não vê um lugar reconhecível pedindo para ser
estendido — só um punhado de condicionais que parecem completos porque
cobrem os campos que existiam quando foram escritos. A correção é dar ao
padrão um NOME (uma função) e um ÚNICO ponto de chamada por campo, para que
"todo caminho do config passa por `resolver_caminho_do_config`" vire uma frase
que se verifica por grep, não uma inferência sobre um bloco de `if`s.

## 5. Como está guardado hoje

O bloco de quatro `if`s virou a função `resolver_caminho_do_config` (em
`crates/phxsql-server/src/config.rs`), chamada explicitamente para os SEIS
campos de caminho do `Config::ler` (`base`, `log_acessos`, `blacklist`,
`dblink`, `jobs`, `backup.destino`) e reaproveitada por
`CifraFio::caminho_da_chave` para o sétimo (`cifra_fio.arquivo`, que continua
resolvendo sob demanda, não elegido para a resolução eager de `Config::ler`,
porque decide também se CRIA o arquivo). Não há uma catraca aqui — o `Config`
tem só sete campos de caminho, medidos um a um; um oitavo campo que chegar
sem passar por `resolver_caminho_do_config` só será pego se alguém repetir
este exercício de leitura, e é essa lacuna que este arquivo registra: a
próxima frente que acrescentar um campo de caminho ao `Config` devia procurar
por este nome de função, não reinventar o bloco de `if`s.
