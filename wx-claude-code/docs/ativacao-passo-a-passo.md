# Ativação do WX Claude Code — passo a passo

Versão 3.47.0. Os comandos são os que existem no pacote; nada aqui é
digitado de memória.

## Do seu lado, uma vez só

**Passo 1.** Descompacte `wx-serial-3.47.0.zip` numa pasta que só você acessa. Ela
nunca vai ao cliente: carrega o emissor de serial.
**Passo 2.** Gere o seu par de chaves:

```bash
python3 emitir.py chaves --saida ~/.wx-serial/chaves
```

A privada fica em `~/.wx-serial/chaves/chave-privada.json`, com permissão
0600. A pública vai para `licenca/chave-publica.json` dentro do plugin
**antes** de empacotar. A que está no repositório é de demonstração, e o
validador avisa enquanto ela estiver lá.

**Passo 3.** Suba o receptor de avisos numa máquina sua, com o SMTP no ambiente:

```bash
export WX_SMTP_HOST=smtp.exemplo.com WX_SMTP_PORTA=587
export WX_SMTP_USUARIO=avisos@exemplo.com WX_SMTP_SENHA='…' WX_AVISO_PARA=voce@exemplo.com
python3 receber.py --porta 8765
```

Sem SMTP ele grava o livro e imprime na tela, mas não envia e-mail.

## A cada venda

**Passo 4.** Se o serial for preso a uma máquina, peça ao cliente o resultado de
`licenca.py maquina` e passe em `--maquina`. Sem isso o serial vale em
qualquer máquina, e a segunda instalação chega a você como possível
recompartilhamento.
**Passo 5.** Emita:

```bash
python3 emitir.py novo --cliente "Loja do Bairro Ltda" --validade 2027-09-08 \
  --email cliente@loja.com --chave-privada ~/.wx-serial/chaves/chave-privada.json \
  --aviso https://seu-servidor:8765/
```

Sai uma linha `WX2.…`. A URL do aviso vai **dentro** do payload assinado:
o cliente não consegue trocá-la sem invalidar o serial.

**Passo 6.** O livro guarda cada emissão, fora do repositório:
- `emitir.py livro` lista;
- `emitir.py reenviar <id>` recupera um serial perdido;
- `emitir.py marcar-revogado <id>` anota a revogação. Anotar **não**
     bloqueia a máquina do cliente; isso está escrito no LEIA-ME e em
     `docs/SEGURANCA.md`.

## Do lado do cliente

**Passo 7.** Instalar e ativar no mesmo comando:

```bash
unzip wx-claude-code-3.47.0-sem-corpus.zip -d ~/plugins
cd ~/plugins/wx-claude-code
./instalar.sh --serial "WX2.…" --corpus ~/Downloads/Help_WL_12k_Json.zip
```

O instalador mostra a `LICENCA.md` e só segue com o aceite. O aceite fica
gravado com o hash dos termos em `~/.wx-claude-code/aceite.json`. No
Windows, `instalar.ps1` com os mesmos parâmetros.

**Passo 8.** Se o plugin já estava instalado, só o serial:

```bash
python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" instalar "WX2.…"
```

Ele pede o aceite se ainda não houver. `--aceito` pula a pergunta e
`--sem-aviso` deixa o aviso pendente.

**Passo 9.** Na instalação o plugin envia **um** aviso ao seu receptor: serial,
empresa, impressão da máquina, versão e data. Nada do projeto, do código
ou dos anexos. Sem rede, o aviso fica em `aviso-pendente.jsonl` e a
instalação segue.

**Passo 10.** Conferir:

```bash
python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" verificar
```

| Estado | O que significa |
| --- | --- |
| `valida` | cliente e validade impressos; tudo roda |
| `ausente` | nenhum serial em `~/.wx-claude-code/licenca` (ou em `$WX_LICENCA`) |
| `vencida` | a data do serial passou; o que já foi convertido continua seu |
| `maquina-diferente` | serial preso a outra máquina |
| `assinatura-invalida` | serial não foi emitido com a chave desta distribuição |
| `formato-invalido` | o texto colado não é um serial `WX2.` |
| `chave-ausente` | o plugin está sem `licenca/chave-publica.json` |

**Passo 11.** A partir daí toda sessão abre dizendo para quem o plugin está licenciado,
    e o `CLAUDE.md` gerado leva a marca d'água. Sem serial válido os comandos
    `/wx-claude-code:*` param na primeira linha e o hook nega os scripts do
    plugin e qualquer escrita em `.wx-migration/`.

## O que chega a você

**Passo 12.** Cada instalação vira um e-mail do receptor:

| Situação | Assunto |
| --- | --- |
| primeira instalação do serial | Instalação — empresa · serial |
| mesma máquina de novo | Instalação repetida — empresa · serial |
| máquina diferente, mesmo serial | POSSÍVEL RECOMPARTILHAMENTO — empresa · serial |
| serial que você não emitiu | desconhecido |

O item 3 da licença é o que você invoca no terceiro caso.

## O que isso protege, e o que não

O serial e os hooks são dissuasão para o cliente honesto: o plugin é texto, e
quem apagar o hook remove a trava. A proteção de verdade é servir o corpus e
os agentes de um servidor seu, com o serial conferido a cada chamada — está em
`docs/SEGURANCA.md`, com o esforço de cada ataque.

## Onde o mesmo texto vive

- Cliente: `licenca/ATIVACAO.md` e `docs/ativacao-do-serial.pdf`.
- Você: `ferramentas/wx-serial/LEIA-ME.md`.
- Em vídeo: `docs/video/wx-claude-code-video-primeiro.mp4`, cenas 2 a 6.
