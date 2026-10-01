# Teste de E/S tem teto de voltas e prazo

**Regra.** Nenhum laço de teste ou de bancada que espera E/S (soquete,
processo filho, arquivo, outra thread) roda sem fim: tem **teto de voltas** e
**prazo**, e estourar o prazo é **reprovação que nomeia a garantia quebrada**,
nunca parada. O executor de guardas tem o seu prazo por cima, mais largo que o
do teste, senão mata a rodada antes de o teste conseguir reprovar.

E o que depende do sistema operacional se prova **contra o sistema
operacional**: queda de conexão por soquete de verdade, falha no meio de um
laço de arquivos por algo que o kernel recusa sozinho (um diretório no lugar
do arquivo), não por gancho de teste no código.

**Cicatrizes.**
- Um laço de prova sem teto fazia ~500 mil trocas em 4 s, martelando o disco
  compartilhado; a janela que ele procurava aparecia nas primeiras voltas.
  O teto (20.000) não a fechou.
- Defeito que **pendura** em vez de falhar (um mutex não reentrante pedido
  duas vezes) travava a bateria inteira.
- Defeito sem fundo (cada nível uma thread nova) só parava no limite de threads
  da máquina: o vermelho se mede no caso limitado que passa pelo mesmo ponto.
- Dez testes unitários passavam; o soquete mostrou que a queda da conexão não
  soltava a reserva. E o próprio teste mentia: `socket.makefile()` segurava o
  descritor, então fechar o soquete não fechava a conexão.
- Linha lida de outro processo: casar o prefixo antes do `\n` aceitava meia
  linha. 0 quedas em 230 corridas sob carga; um servidor falso que escreve em
  dois pedaços derruba o teste toda vez.
