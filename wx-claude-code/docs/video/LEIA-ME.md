# Vídeo de uso

`wx-claude-code-video-de-uso.webm` (1280×720, VP8, 3 min 38 s): vinte e nove
cenas, da instalação ao resultado em Rust, cada uma reproduzindo uma saída real
de sessão do Claude Code ou de script do
plugin, as mesmas capturas dos prints em `../prints/`. As quatro últimas
fecham o arco que dá sentido ao resto: a bateria pesada de cenários, a
procedure WLanguage lida do PDF do legado, o Rust gerado por uma sessão real
citando a página de origem dentro do código, e o `cargo test` que prova a
regra — com o que mudou de semântica dito, não escondido. O comando aparece
digitado, a saída aparece linha a linha, e nada foi inventado ou editado.

O `.mp4` (H.264, mesmo conteúdo) foi convertido do WebM com um ffmpeg estático com libx264 (`imageio-ffmpeg`).

Para regravar: `node gravar-video.mjs <pasta-de-saida> <pasta-das-capturas>`.
O script usa o Playwright do ambiente e grava em WebM porque é o único codec
de vídeo que o ffmpeg do Playwright codifica; para MP4, converta com um ffmpeg
que tenha libx264.

## Segundo vídeo: de PHP para Rust

`wx-claude-code-video-php.webm` (1 min 26 s, 11 cenas) mostra o outro caminho,
o que prova a parte **E/OU** da regra do legado: um sistema PHP procedural de
2009, sem nada de WINDEV, atravessando o plugin inteiro — instalação, o serial
recusando e depois liberando, o questionário, o portão G0 aceitando um projeto
sem um único PDF, a conversão para Rust por uma sessão real, o `cargo test`
contra o golden master capturado do próprio legado, a entrega com hashes e o
registro de operações.

Os dois roteiros vivem no mesmo gerador:

```bash
node gravar-video.mjs <pasta-saida> <pasta-capturas>        # roteiro 'uso'
node gravar-video.mjs <pasta-saida> <pasta-capturas> php    # roteiro 'php'
```

## O terceiro vídeo: a bateria de testes

`wx-claude-code-video-bateria.mp4` (roteiro `bateria`) mostra as sete provas
rodando: `tests/testes.py -v`, `tests/cenarios.py`, `tests/fluxo.py`, o
validador estrito, `claude plugin validate`, `cargo test` do wx-modelos e o
`atualizar-paginas.py --conferir`. Cada cena é a saída real do comando,
gravada no shell **imediatamente antes** da gravação — nenhuma linha é digitada.

Para regravar, capture de novo e rode o gravador com a pasta das capturas:

```bash
C=/tmp/caps-bateria; mkdir -p $C
{ echo '$ python3 tests/testes.py -v'; python3 tests/testes.py -v 2>&1 | grep -E '\.\.\. ok$' \
    | sed -E 's/ \(__main__\.[A-Za-z0-9_.]+\)//' | head -40; echo '…'; python3 tests/testes.py 2>&1 | tail -3; } > $C/testes-v.txt
# idem: cenarios, fluxo, validador, plugin-validate, cargo, paginas (ver gravar-video.mjs)
node docs/video/gravar-video.mjs /tmp/saida $C bateria
```

Dois retoques saíram de **olhar os quadros**, não de ler o código: o `-v` do
unittest imprime `(__main__.Classe.nome)` e as linhas quebravam — o `sed`
acima tira; e `... ok` não saía verde, porque a regra de cor só conhecia os
marcadores dos outros roteiros.

Um defeito real do gravador apareceu aqui: ele avaliava os três roteiros ao
carregar, então gravar a bateria com uma pasta que só tinha as capturas dela
quebrava no `cap('45-instalacao')` do roteiro de uso. Captura ausente agora vira
sentinela no carregamento e **erro com o nome** só para o roteiro escolhido.


## O quarto vídeo: o primeiro projeto

`wx-claude-code-video-primeiro.mp4` (roteiro `primeiro`) é o plugin usado de
ponta a ponta pela primeira vez, do zelador à entrega: o pacote do cliente
descompactado, o par de chaves no vendedor, o serial emitido, a instalação
com a licença aceita e o aviso chegando ao vendedor por e-mail, e então um
mini CRUD PHP + MySQL de **uma tabela** virando Rust (`tiny_http` + `mysql`)
com tela React 19 + Vite sobre o mesmo MySQL. O golden master é capturado
rodando o próprio legado, o Rust reproduz os 5 casos, a tela é exercitada
pelo Playwright, o grafo fecha em zero — e o defeito achado olhando a tela (os
botões vazando da tabela) aparece com o conserto e a prova refeita, porque foi
o que aconteceu. As capturas são as saídas reais da sessão em que o projeto
foi feito, com os caminhos da máquina de trabalho trocados por `.` e `~`.

O projeto inteiro (entrada, questionário e destino) está em
`../../exemplos/clientes-php-mysql/`.

```bash
node gravar-video.mjs <pasta-saida> <pasta-capturas> primeiro
```

## O quinto vídeo: dos PDFs do WINDEV ao Rust + React

`wx-claude-code-video-windev.mp4` (roteiro `windev`) é o caminho principal do
plugin exercitado de ponta a ponta: o exemplo ESTOQUE (WINDEV 2025, quatro
PDFs, sem projeto nativo) virando Rust + Axum + PostgreSQL 16 com a tela
WIN_Venda em React 19. O G0 diz FORENSIC, cada PDF vira Markdown com a página,
o golden master vem da amostra (10/10, inclusive a query sobre a amostra
migrada), o Rust cita a página do PDF em cada regra, a tela reproduz os quatro
estados do PDF de interfaces (16/16 pelo Playwright), e o grafo fecha com uma
lacuna só: o GAP plantado no exemplo, `EstornaEstoque`, que o PDF de código
não tem. O destino inteiro está em `../../exemplos/estoque-wx/destino/`.

```bash
node gravar-video.mjs <pasta-saida> <pasta-capturas> windev
```

## O sexto vídeo: o passo a passo na versão atual

`wx-claude-code-video-passos.mp4` (roteiro `passos`) é a lista do manual
virando saída real: dezoito comandos, do `instalar.sh --conferir` ao zelador,
rodados nesta versão no momento da gravação sobre o exemplo ESTOQUE. O que o
C-GATE achou no caminho — as linhas «verified» da matriz sem `test_result_ref`
— foi corrigido no exemplo antes, e a saída mostrada é a de depois do
conserto; a captura do golden foi refeita quando o PostgreSQL estava fora do
ar, porque 9/10 por banco parado não é o número do plugin.

```bash
node gravar-video.mjs <pasta-saida> <pasta-capturas> passos
```

## O sétimo vídeo: a ativação passo a passo

`wx-claude-code-video-ativacao.mp4` (roteiro `ativacao`) mostra os dois lados
da licença rodando de verdade no pacote recém-empacotado: o vendedor gera o
par de chaves, sobe o receptor e emite o serial com a URL do aviso dentro da
assinatura; o cliente vê «ausente», instala com o aceite e vê «valida»; o
receptor recebe a primeira instalação e depois a segunda máquina; o livro
reenvia e anota a revogação; e o mesmo serial numa distribuição com outra chave
dá «assinatura-invalida». A cena da segunda máquina é um aviso com outra
impressão enviado à mão ao receptor, e a legenda diz isso.

```bash
node gravar-video.mjs <pasta-saida> <pasta-capturas> ativacao
```
