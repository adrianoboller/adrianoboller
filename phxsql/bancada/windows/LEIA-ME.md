# A prova do binário Windows — sob `wine`

O irmão da `bancada/arm/`, e pelo mesmo motivo: **compilar não é rodar**.

Até esta rodada, a §6 do `docs/EMPACOTAMENTO.md` dizia, com todas as letras:

> O que **não** dá: rodar. Sem Windows e sem `wine`, o `.exe` é conferido pela
> forma (PE32+ x86-64), pelas DLLs que importa e pelos símbolos que exporta —
> nunca por execução. Dizer mais que isso seria inventar.

Estava certo enquanto durou. Com o `wine` instalado, «compila e empacota»
virou **«gravou e leu 50 linhas»**.

```bash
sudo apt install wine        # ~150 MB de download, ~700 MB em disco
bancada/windows/provar.sh
```

## Por que não precisou de VM

Pela mesma razão do ARM, e por um caminho diferente. Uma VM completa exige
`/dev/kvm`, que esta máquina não tem — ela própria é uma máquina virtual sem
aninhamento.

- No ARM, o `qemu-user-static` emula o **binário**: traduz instrução ARM por
  instrução x86. Não precisa de KVM porque não emula máquina nenhuma.
- No Windows, o `wine` **não emula nada**. O `.exe` é x86-64 e esta máquina é
  x86-64: o código roda **nativo**. O que o `wine` reimplementa são as **DLLs**
  do Windows (`kernel32`, `ws2_32`, `advapi32`…) sobre a libc do Linux.

A diferença importa na hora de ler o número: o tempo do ARM carrega a tradução
instrução a instrução; o do Windows, não.

## O que a prova faz

Os mesmos passos da do ARM, com a **mesma sonda** (`bancada/arm/sonda.py` —
uma só, porque o trabalho provado é o mesmo; só o rótulo vem de fora, por
`ALVO` e `MODO`):

| Passo | O que prova |
|---|---|
| `phxsqld.exe --exemplo 1` | o `.exe` sobe e escreve o config modelo |
| `phxsqld.exe --senha` | o **PBKDF2** roda sob `wine` |
| subir o servidor | a porta abre — `ws2_32` do `wine` serve o nosso soquete |
| `login` com o hash que o próprio `.exe` gerou | a criptografia fecha dos dois lados |
| `criar_database` + `criar_tabela` | o `CreateFile`/`WriteFile` do `wine` cria os sete arquivos |
| 50 `inserir` | grava de verdade, com índice e CRC |
| `varrer` | **50 registros, 50 devolvidas** |

## Três decisões que o script tomou, e por quê

**O binário pode sair do PACOTE.** Sem `target/x86_64-pc-windows-gnu` (recém
limpo, por exemplo), o script pega o `.exe` de `pacotes/phxsql-*-windows.zip`.
É até melhor: o que se prova aí é **o arquivo que o usuário baixa**, e não um
subproduto da compilação que ninguém distribui.

**O prefixo do `wine` mora no descartável.** `WINEPREFIX` aponta para dentro do
`mktemp -d` da corrida, e some com ela. Um `~/.wine` com estado de outro dia é
exatamente o tipo de coisa que faz a prova passar por engano.

**Quem subiu se confere pela PORTA, não pelo PID.** O `wine` troca o processo:
o PID que o shell lançou morre, e `kill -0` nele diz «não subiu» com o servidor
no ar. A prova de que subiu é a porta respondendo.

E o corolário, que custou uma medição errada: o **RSS** também não sai do PID
do lançador nem do primeiro `/proc` cuja linha de comando casa com
`phxsqld.exe`. A primeira versão fazia assim e o número pulou de **6.148 kB**
para **17.236 kB** entre duas corridas iguais — às vezes achava o lançador do
`wine`, às vezes o servidor. Hoje sai do processo que **é dono do soquete**,
achado pelo inode em `/proc/net/tcp`, e ficou estável: **8.796 e 8.800 kB** em
duas corridas. *Número que muda 3× sem nada mudar não está medindo o que diz.*

## O que esta bancada NÃO prova

**Desempenho.** As `50 linhas` saíram em 4,5 ms/linha numa corrida e 60,3 ms
noutra, com a máquina carregada no meio. Isso não é o custo do `wine` nem o do
motor — é ruído. Para desempenho no Windows não há substituto: **é preciso um
Windows**.

**Compatibilidade completa.** O `wine` reimplementa as DLLs; ele não *é* o
Windows. Um `.exe` que roda sob `wine` quase sempre roda no Windows, mas o
«quase» é real, e o caminho que este script exercita é estreito de propósito:
arquivo, soquete, relógio e criptografia. É o caminho do servidor — e é o que
não estava provado antes.

**A interface web.** Fica desligada na corrida. Página servida é outro assunto,
e ela já tem prova própria em `bancada/bateria/prova-tela.mjs`.
