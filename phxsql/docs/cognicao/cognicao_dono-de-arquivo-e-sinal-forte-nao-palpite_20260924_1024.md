# Dono de arquivo é sinal FORTE, não um palpite como data ou conteúdo

**Estado:** INFRUTÍFERO
**Causa:** a hipótese «dono diferente do processo e do parceiro = terceiro»
tomou o sinal que não se FORJA pelo sinal que IDENTIFICA o intruso. O root
também é outro dono, e é o administrador: na instalação do MANUAL (§7.4),
`sudo 7z x` para trocar um token vazado deixa o `.json` do root ao lado do
`.phz` do serviço, e o serviço subia do `.phz` VELHO com o token revogado
valendo — a revisão SEC provou pelo sistema operacional, e o teste
`crates/phxsql-server/tests/config-phz.rs::o_json_do_root_ao_lado_do_phz_do_servico_recusa_o_arranque`
cai com esta regra reposta («ainda rodava depois de 20 s: subiu como
servidor», o servidor como uid 65534). O sticky bit, que é o que torna um nome
alheio «plantado», nem era conferido. E a régua citada media outra coisa:
MySQL e MariaDB ignoram por MODO (gravável por todos); medido, o `mysqld`
8.0.46 LÊ um `my.cnf` de outro dono com 0644.
**Prevenção:** antes de usar um metadado como prova de intruso, liste QUEM
MAIS produz o mesmo sinal legitimamente (o root, o dono da pasta, o próprio
serviço) e exija a condição do sistema operacional que torna o sinal exclusivo
do intruso (aqui: sticky bit E pasta gravável por outros, com o root e o dono
da pasta fora da conta de terceiro); e ao citar outro motor na régua, cite o
CRITÉRIO dele medido pelo binário, não só o comportamento.

## O que aconteceu

Pedido 481: o `config_phz::resolver` recusava o arranque (`PhxError::Conflito`,
«conflito de escrita») sempre que `config.json` **e** `config.phz` existissem,
mesmo quando um terceiro — numa pasta com *sticky bit* como `/tmp` — criava um
dos dois sem poder tocar no outro. A regra de sempre («fonte de verdade
ambígua não se resolve por palpite — nem pela data») virou negação de serviço
nova para um servidor que nunca escreveu nada ali.

Duas hipóteses, medidas contra os quatro motores (item obrigatório da cláusula
«o pesquisador decide»):

- **H1** — recusar sempre que a pasta tiver *sticky bit* e for gravável por
  outros, no molde do PostgreSQL. Medido pelo binário (16.13, `initdb` +
  `chmod`): `FATAL: data directory "…" has invalid permissions` /
  `has wrong ownership`, incondicional, mesmo sem ataque em curso.
- **H2** — ignorar (e avisar) o arquivo cujo dono não é nem o do processo nem
  o do parceiro do par, no molde do MySQL/MariaDB. Medido pelo binário
  (MySQL 8.0.46 e MariaDB 11.8.9 real em container): `[Warning] World-writable
  config file '…' is ignored`, e o processo SOBE ignorando só aquele arquivo.

## O que eu concluí primeiro, e estava errado

A primeira leitura tratou a comparação de dono entre os dois arquivos do par
como uma variante do que a pétrea já proíbe («não decide por palpite, nem pela
data») — quase descartei H2 achando que compará-los por dono seria reabrir a
mesma porta que `cp -p` (que preserva `mtime`) já tinha ensinado a desconfiar.
Cheguei a escrever, num primeiro rascunho, que H2 «escolheria por metadado
igual ao que a data já mostrou não servir».

Isso confundia DOIS metadados de força muito diferente. `mtime`/conteúdo são
sinais **fracos**: qualquer um com permissão de escrita no arquivo os forja
(`cp -p`, um `touch`, restaurar um backup). Dono de arquivo é um sinal
**forte**: no POSIX, só quem já É o dono ou é `root` troca o dono de um
arquivo (`chown`). Um atacante sem privilégio, na pasta de *sticky bit* do
cenário do pedido, não consegue forjar posse — só consegue CRIAR um arquivo
seu, que carrega o UID dele de verdade. A pétrea nasceu contra o sinal fraco;
não alcança o forte.

## O que a medição disse

- Média ponderada da régua do dono (PG 4, MariaDB 3, MySQL 2, SQLite sem
  dado): MariaDB 3 + MySQL 2 = **5** pelo padrão "auditar e ignorar" contra
  PostgreSQL 4 pelo "auditar e recusar" — **5 > 4**.
- Mas as duas medições respondem a PERGUNTAS diferentes: o PostgreSQL audita
  a permissão do **diretório inteiro**, sempre (a ameaça dele é dado cru
  exposto por fora do SQL); o MySQL/MariaDB auditam o **arquivo específico**
  e só ignoram aquele. O nosso `config.json` já nasce `0600`; a única
  exposição da pasta aberta é a possibilidade de um arquivo plantado. A
  leitura qualitativa aponta para o mesmo lado do número: H2.
- Prova real (não só leitura): `phz-de-um-terceiro-e-ignorado...` cai
  (vermelho) com `if dc == processo && dp != processo` trocado por
  `if false`, e passa (verde) revertido — `config_phz.rs`, catálogo
  `config-phz-terceiro-nao-e-ignorado`.

## A regra

**Ao decidir se um metadado pode substituir «escolher por palpite», pergunte
quem PODE forjá-lo sem privilégio — não se ele já foi usado como exemplo de
palpite em outro contexto.** Data e conteúdo caem porque qualquer leitor com
`write` os copia; posse de arquivo não cai pela mesma razão, porque trocar
dono exige ser o dono ou ser `root`.

## Como está guardado hoje

A regra desta cognição **saiu do código** na volta seguinte (revisão SEC do
481, BLOQUEIO): `resolucao_do_par` e o `dono_do_processo` por arquivo-sonda
deram lugar a `terceiro_no_par` (`crates/phxsql-server/src/config_phz.rs`),
com o root nunca terceiro, o sticky bit e a pasta gravável por outros
exigidos, e o dono da pasta fora da conta. A régua refeita está em
`docs/SEGURANCA.md` §23, e a guarda que repõe esta regra inteira é a
`config-phz-par-root-vira-terceiro` (`bancada/guardas/catalogo.py`).
