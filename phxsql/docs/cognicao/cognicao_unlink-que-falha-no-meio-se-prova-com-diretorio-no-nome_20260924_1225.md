# O `unlink` que falha no meio se prova com um diretório no nome, e não com gancho de teste

**Estado:** PENDENTE

*24/09/2026, 12:25 — pedido 368, achado A5 do papel C (o erro do expurgo
perdia a lista parcial).*

## 1. O que aconteceu

A fase 3 do expurgo da trilha (`TrilhaFile::apagar_expurgados`) confere o
bilhete de **todos** os volumes e só então apaga um a um. O parecer pediu que o
`unlink` que falha no meio devolvesse, no erro, os volumes que já tinham
saído. Para provar nos dois sentidos era preciso um `unlink` que falhasse
**depois** de o bilhete do mesmo volume ter conferido — e a função é um bloco
só, sem ponto entre as duas coisas onde o teste possa entrar.

## 2. O que eu concluí primeiro, e estava errado

Que a prova pedia um gancho `#[cfg(test)]` no `Volumes::apagar_volume` (um
`thread_local` com o caminho que deve falhar). Funcionaria, e provaria o
gancho, não o sistema operacional — e é mais uma peça de código de teste
dentro do motor para alguém esquecer ligada no espelho.

## 3. O que a medição disse

O descritor que o **plano** abriu continua lendo o arquivo depois de um
`rename`: o inode é o mesmo. Então o teste renomeia `t_002.lgpd` para fora e
cria um **diretório** com o nome dele. A listagem acha o nome (o diretório
existe), o bilhete confere pelo descritor antigo, o volume 1 sai, e o
`unlink` do 2 devolve `EISDIR` (os error 21). Com o defeito reposto (o erro
subindo cru por `?`) a mensagem é só «Is a directory (os error 21)» e a
asserção cai; com o conserto ela diz «parou no volume 2 (…); ja tinham saido
[1]», e o diretório mostra `t.lgpd`, `t_002.lgpd`, `t_003.lgpd`.

## 4. A regra

Para provar falha no MEIO de um laço de arquivos, procure o que o sistema
operacional recusa sozinho — diretório no lugar do arquivo, descritor aberto
antes do `rename` — antes de pôr gancho de teste no motor.

## 5. Como está guardado hoje

`crates/phxsql-store/src/trilha.rs::o_unlink_que_falha_no_meio_diz_o_que_ja_saiu`.
O limite: a prova depende de o plano deixar o descritor do volume 2 aberto no
cache do `Volumes`. Se o cache mudar, o teste cai no bilhete (erro diferente,
asserção da frase reprova) — cai alto, não passa por engano.
