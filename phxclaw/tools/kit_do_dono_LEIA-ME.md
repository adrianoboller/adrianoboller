# Kit do dono: os portões que só fecham na sua máquina Windows

Gerado por `tools/kit_do_dono.sh`, não montado à mão. Três pastas, e cada uma fecha um item.

## 1. `dispositivos/`: pareamento no Windows real

O que já está provado: Linux entre processos, e estes mesmos dois `.exe` rodando no Wine 9.0,
inclusive com o Credential Manager. O Wine não é Windows, e por isso falta você.

1. Dê dois cliques em `1-servidor.bat` e deixe a janela aberta. Deve aparecer
   `dispositivos em wss://127.0.0.1:8788 (1 token(s) de pareamento)`.
2. Dê dois cliques em `2-parear.bat`. Deve aparecer `pareado` e depois
   `sessao … cerca 2`. Feche essa janela; o servidor continua aberto.
3. Dê dois cliques em `2-parear.bat` **de novo**. Tem de **recusar**, com «token de
   pareamento invalido ou ja usado», porque o token é de uso único.
4. Dê dois cliques em `3-religar.bat`. Tem de aparecer `sessao … cerca 3`, sem token,
   pela chave guardada no Credential Manager. A cerca sobe a cada sessão.

Mande o texto das três janelas. Os três resultados juntos são a prova.

O Windows pode pedir permissão de firewall para o `phxclaw.exe`: tudo roda em `localhost`,
e a porta não precisa ficar aberta para fora.

A `ca.pem` e o certificado são de **teste**, valem 30 dias e só para `localhost`. A chave da CA
não vem no kit.

## 2. `u4b-wlanguage/`: compilar as regras no WinDev

O `Regras.wl` sai do mesmo modelo que gera o crate Rust, que já está compilado e testado. O
teste de paridade garante que as mensagens são as mesmas nos dois. As 12 funções WLanguage
emitidas foram conferidas no Help 2026, mas nunca passaram por um compilador.

Siga o `LEIA-ME.md` da pasta: importe o `pedidos.sql` na análise, declare as chaves,
cole o `Regras.wl` numa coleção de procedimentos e compile.

Mande os erros de compilação, se houver, com o número da linha. Cada erro vira um conserto no
gerador, com teste, e não no arquivo gerado.

## 3. `desktop/`: teclado, mouse, captura e shell governado

Dê dois cliques em `prova-desktop.bat` e não toque em nada por uns 15 s. A prova:

- confere que a política padrão **nega** lançar programa e que `cmd.exe` negado é recusado;
- captura a tela e confere que a imagem não é de uma cor só, gravando
  `prova-desktop-captura.png` ao lado;
- move o mouse ao centro e confere a posição que o Windows devolve;
- abre o Bloco de Notas pelo executor governado, digita um código aleatório, salva com
  Ctrl+S e confere o código no arquivo gravado.

Tem de terminar em `placar: 4/4`. Mande o texto da janela. No Wine deu 4/4; e digitando
outro texto no lugar do código, deu 3/4, prova de que a checagem não passa sozinha.

## O que fica fora deste kit

- **macOS, Android e iOS**: precisam de compilação nativa em cada plataforma, que não se
  faz a partir deste contêiner.
