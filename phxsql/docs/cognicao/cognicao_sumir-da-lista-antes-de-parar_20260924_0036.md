# Sumir da lista antes de parar: a janela dizia «desligada» com a porta presa

## O que aconteceu

No programa de mesa do phxvpn (24/09/2026), a prova com duas janelas fazia:
ligar com «lembrar a senha», desligar, e religar sem digitar a senha. Isolado,
com um computador só, funcionava. Com um par conectado, o clique em «Ligar»
não fazia nada visível. O rodapé continuava «rede Matriz desligada».

## O que eu concluí primeiro, e estava errado

Que o clique se perdia porque a janela redesenhava a lista inteira a cada 2 s,
e o botão era trocado debaixo do mouse. O redesenho é mesmo um risco, e o
conserto dele ficou: só redesenha quando algo muda. **Mas não era a causa.** A
falha continuou idêntica depois do conserto.

## O que a medição disse

Instrumentar os pedidos da página mostrou o que a tela escondia:

```text
PEDIDO   /api/ligar
RESPOSTA 400 porta UDP 51820: Address already in use
RESPOSTA 200 rede Matriz desligada      <- chegou DEPOIS
```

O `desligar` tirava a rede do mapa **antes** de o nó terminar. A consulta de 2 s
já via «desligada» enquanto a porta UDP continuava presa, e o erro do religar
sumia sob a mensagem do desligar, que chegava depois. Com um par conectado, o
nó levou **989 ms** para parar; sem par, parava rápido e a corrida não aparecia.

## A regra

Um recurso só sai da lista de «em uso» depois de liberado de fato. Enquanto
não sair, o estado é «desligando», e a tela o mostra.

## Como está guardado hoje

`Mesa::desligar` espera o fio do nó terminar para então remover a rede, e
`ligar` recusa «ainda está desligando». O botão mostra «Desligando…»
desabilitado. **Não há teste automatizado**: a corrida precisa de placa TUN e
de um par vivo, e só se prova no roteiro com `ip netns` e Playwright. Lição
de método: o segundo palpite só apareceu quando se olharam os PEDIDOS, não a
tela.
