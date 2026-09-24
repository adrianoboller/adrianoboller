# O Wine roda a suíte compilada para Windows no Linux — `cargo check` não era o teto

## O que aconteceu

O P2P do phxvpn ganhou o código do Windows (TAP-Windows6 por FFI) e, antes
disso, o núcleo ganhou o `BCryptGenRandom` do conserto C2. Nos dois casos, a
única prova registrada era `cargo check --target x86_64-pc-windows-gnu`.

## O que eu concluí primeiro, e estava errado

Que, sem uma máquina Windows, o teto da prova era o `cargo check` — «compila,
não rodado». Escrevi isso em dois commits e em um documento. Mas o `check`
**não linka**. Um nome de biblioteca errado no `#[link(name = "bcrypt")]` ou
uma assinatura `extern "system"` torta passariam por ele, e o `BCryptGenRandom`
nunca teria executado uma vez.

## O que a medição disse

Com o MinGW como linker, o `.exe` linkou, e o `objdump` listou as DLLs: só
as do Windows, nenhuma nossa. Com o Wine como `runner` do cargo
(`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER`), rodaram a suíte do phxvpn
compilada para Windows (**56/56**, com P2P por UDP real) e a do `phxsql-core`
(**379/379**). O `BCryptGenRandom` executou.

## A regra

Código de Windows se prova no Linux até onde o Wine vai: linkar com o MinGW,
listar as DLLs e rodar a suíte com o Wine como runner. «Não rodado» só vale
para o que o Wine não tem, como um driver de kernel.

## Como está guardado hoje

Na seção «P2P no Windows» do `phxvpn/docs/PHXVPN.md`. **Não há portão que
rode isso sozinho**: MinGW e Wine foram instalados à mão neste contêiner e
somem com ele. O driver TAP continua sem prova, e o `prova-windows.ps1` é o
roteiro para uma máquina real.
