# O vermelho saiu pela guarda vizinha, e escondeu o dano

**Data:** 24/09/2026, 01:40 (UTC) · **Frente:** pedido 372, o `dblink.json`
cifrado com chave mestra externa · **Papel:** B, com a prova real do F.

## 1. O que aconteceu

A primeira versão do teste `com_a_chave_o_disco_nao_guarda_a_senha_nem_o_token_em_claro`
gravava duas ligações e **só depois** contava a senha e o token no arquivo.
Com o defeito reposto — o `para_disco` recebendo o selo e escrevendo o campo de
sempre — o teste ficou vermelho, e o vermelho foi este:

```
called `Result::unwrap()` on an `Err` value: Esquema("a credencial senha da
ligacao \"loja\" veio CIFRADA do cadastro, e esta gravacao nao tem com o que
selar: grava-la em claro seria rebaixar o arquivo")
```

Na **segunda** gravação. Quem recusou foi outra guarda da mesma frente — a que
impede escrever em claro uma credencial que veio selada —, disparada porque a
primeira gravação, com o defeito, marcou a senha como selada na memória depois
de escrevê-la em claro no disco. O dano (a senha em claro no arquivo, desde a
primeira gravação) **nunca foi medido**: o teste morreu antes de chegar à conta.

O mesmo desenho apareceu uma segunda vez, no sentido contrário: com a recusa do
rebaixamento tirada, o teste da chave errada continuou passando a conta do dano
dele (nenhuma ligação trancada para sempre — a senha em claro abre com
qualquer chave) e só caiu no veredito «a gravação foi aceita». O dano daquele
defeito era outro — texto puro dentro do cadastro cifrado — e o teste não o
media.

## 2. O que eu concluí primeiro, e estava errado

«Caiu com o defeito reposto, então a prova real está feita.» Estava errado nos
dois casos. No primeiro, caiu pela porta da **guarda vizinha**: o que o
vermelho provava era a recusa do rebaixamento, e não a guarda que eu tinha
tirado. No segundo, caiu pelo **veredito**, e a regra da casa pede o dano.

## 3. O que a medição disse

Depois de reescrever o teste para medir o disco **depois de cada gravação e
antes do veredito dela**, o mesmo defeito deu:

```
a gravacao 1 COM chave mestra deixou a credencial em claro no dblink.json:
a senha 1 vez(es) e o token 0 vez(es), em 522 bytes
```

e o teste da chave errada ganhou a conta do texto puro no disco antes do
veredito. Os oito defeitos repostos da frente caíram, cada um, pelo dano que é
dele — os oito estão no `catalogo.py` (372.1 a 372.8).

## 4. A regra

**Em teste de várias gravações, meça o dano depois de CADA uma e antes do
veredito dela — e cada teste no `caem` de uma guarda tem de medir o dano
DAQUELA guarda: vermelho que sai pela guarda vizinha prova a vizinha.**

## 5. Como está guardado hoje

Nos dois testes reescritos e nas entradas 372.1 e 372.7 do catálogo, que dizem
por quê. **Não há conferidor para isso**: nada na casa lê a mensagem do
vermelho e diz se ela mede dano ou veredito, ou se saiu pela guarda certa. É
revisão — e o buraco fica nomeado aqui.
