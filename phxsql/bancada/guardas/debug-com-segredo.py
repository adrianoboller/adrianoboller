#!/usr/bin/env python3
"""A catraca do `Debug` com segredo dentro -- a regua da lei que a guarda so exemplifica.

    python3 bancada/guardas/debug-com-segredo.py --catraca      autoteste, mede, compara ao teto
    python3 bancada/guardas/debug-com-segredo.py --numeros      para o docs/qa/medir.py
    python3 bancada/guardas/debug-com-segredo.py --autoteste    a prova real da propria regua
    python3 bancada/guardas/debug-com-segredo.py --inventario   tudo o que o crivo ve, em tres grupos
    python3 bancada/guardas/debug-com-segredo.py --raiz DIR     mede outra arvore (a prova reposta)

# O defeito que motivou

Em 16/09/2026 uma frente achou NOVE estruturas, em tres crates, derivando
`Debug` com segredo dentro -- 14 campos: `Definicao`, `Config`, `Origem`,
`Cluster`, `Email`, `Rest`, `Usuario`, `Comando`, `Receita`. Um `{:?}` no
`Config` despejava oito segredos de uma vez. Consertadas em `74de67e`.

E a guarda JA EXISTIA: o catalogo tinha `debug-da-cifra-mostra-a-senha` desde
a frente G-CRIPTO, com a lei inteira escrita no `porque`. Ela travou UMA
struct, nao a lei -- e nada impedia a decima de nascer derivando. Guarda que
repoe defeito trava a ESTRUTURA onde o defeito foi reposto; quando o mesmo
defeito cabe em mais de um lugar, o que protege a lei e uma REGUA que conta os
lugares (`docs/cognicao/cognicao_guarda-trava-a-struct-nao-a-lei_20260917_0010.md`).
Esta e a regua; a guarda passa a ser o exemplo dela.

# O crivo, em tres partes -- e por que cada uma sozinha erra

`grep senha` acha 40 campos em `crates/`, e 26 NAO sao defeito (`docs/SEGURANCA.md`
§16.1). O crivo que separa:

1. o NOME casa o lexico (`LEXICO`, abaixo) -- e nao casa quando o proprio nome
   nega o segredo (`NEGA_NO_NOME`: `senha_env` e o nome da variavel de
   ambiente; `chave_publica` e publica);
2. o TIPO e portador de valor (`String`, `Vec<u8>`, `[u8; N]`, `Option<String>`
   e os envoltorios obvios -- `portador()`), o que mata `usa_senha: bool` -- e
   um tipo TAMBEM e portador quando o `impl Debug` de QUEM O USA o redige
   (nunca por nome cravado: `Segredo` entra assim, pedido 477);
3. o valor E MESMO segredo -- e isso so se decide LENDO o campo. `Botao.chave`
   e um seletor de CSS; `chave_do_fio` e a chave PUBLICA do pino. Nenhum
   casador de texto decide isso, entao os falsos positivos ficam DECLARADOS em
   `ISENTOS`, cada um com o motivo lido no fonte. Lista escondida e vazamento
   futuro com cara de verde; lista visivel e uma linha que alguem le.

O que passa pelas tres CONTA quando a struct deriva `Debug` -- ou quando o
`impl Debug` escrito a mao LE o campo (`.field("senha", &self.senha)`), que e
o mesmo defeito por outro caminho: e literalmente a troca da guarda
`debug-da-cifra-mostra-a-senha`. A regua enxerga os dois.

# Tipos portadores descobertos, nao cravados (pedido 477)

`Segredo` (`config.rs`) tem um campo (`valor: String`) que NAO casa o lexico --
a parte 1 nunca o alcancaria pelo NOME dele. O que prova que `Segredo` E
portador nao e o proprio tipo -- e QUEM O USA: `Cifra.senha`, `Email.senha`,
`CifraFio.chave_privada`, `Definicao.senha`/`token` e `CifraDoDblink.segredo`
sao campos cujo NOME casa o lexico, e o `impl Debug` a mao de cada DONO redige
esse campo (literal ou `campo: _`, nunca o le cru) -- prova, de FORA, que o
TIPO do campo carrega o segredo sozinho. `tipos_portadores_descobertos()`
varre essa prova pelos `impl ... Debug for X` manuais e devolve o CONJUNTO de
nomes; nenhum e digitado, e o `Segredo` entra por ela e nao por nome cravado.

Um tipo descoberto entra em DOIS lugares do crivo:

* como TIPO, em `portador()` -- um campo `outro: Segredo` (ou `Option<Segredo>`)
  conta pela parte 2 como qualquer `String`, MESMO que o nome do campo nao
  case o lexico: a struct que o contem DERIVANDO `Debug` ja e vazamento,
  porque o derivado chamaria o `Debug` do `Segredo` -- que pode nem proteger
  mais;
* como DONO, nos CAMPOS do proprio tipo descoberto -- `Segredo.valor` passa a
  contar mesmo o nome "valor" nao casando o lexico, porque a estrutura
  INTEIRA so existe para carregar o segredo (e o proprio tipo DERIVAR
  `Debug`, ou seu `impl` a mao passar a LER o campo cru, conta do mesmo jeito
  -- e o defeito da guarda `debug-do-segredo-mostra-o-valor`).

E a leitura, para um campo de TIPO descoberto, nao e a mesma regra da parte 3:
passar o valor INTEIRO adiante (`.field("segredo", segredo)`, em
`CifraDoDblink`) e SEGURO, porque quem imprime dali e o `Debug` PROPRIO do
tipo -- a mesma camada que protege em toda parte. O que vaza e ir ALEM do
valor bruto: um `.` logo depois do identificador (`self.senha.valor()`,
literalmente a troca de `debug-da-cifra-mostra-a-senha`) -- `le_o_valor_do_campo()`
so acusa isso. Sem essa distincao, `CifraDoDblink.segredo` seria um falso
positivo eterno so por passar o `Segredo` adiante.

# O que ela NAO ve, declarado

* Segredo em campo cujo NOME nao casa o lexico: `Direcao { k: [u8; 32] }` no
  `fio.rs` e a chave de sessao e a regua nao a alcanca (hoje `Direcao` nao
  deriva `Debug`; o `Transporte` que a carrega esconde as duas). Nome e a
  primeira parte do crivo e nao ha como le-lo por outra via.
* `impl Display` (ou qualquer outro formatador) que imprima o segredo: e ato
  deliberado, nao derivado pelo compilador; ninguem cobre hoje.
* `impl Debug` que chegue ao segredo por caminho indireto (um metodo, um
  `let Chave(x) = self` numa struct de tupla): a regua olha o identificador do
  campo, nu, no corpo do `impl`. Basta para o defeito que existiu.
* `crates/*/examples/` e `crates/*/tests/`: fora da regua, de proposito -- nao
  e codigo que sobe com o servidor. Medido em 17/09/2026: zero `derive(Debug)`
  com segredo la tambem.
* Tipo descoberto por impl alheio PRECISA de pelo menos UM dono que o redija
  HOJE (literal ou `campo: _`): se toda struct que usa um tipo custom so o
  PASSA adiante (nunca o substitui nem descarta), a regua nunca aprende que o
  tipo carrega segredo. E a descoberta e por NOME cravado no `impl`, sem
  caminho de modulo: duas structs de arquivos diferentes com o mesmo nome
  contam como o MESMO tipo portador -- inofensivo hoje (o `Segredo` de
  `phxsql-store/src/cofre.rs`, sem relacao com o do `config.rs`, ja tinha o
  unico campo portador dele pego pelo NOME), mas e o motivo de a lista viver
  em `analisar()` e nao virar constante: ela e, na pratica, por nome cravado
  tambem, so que o nome sai do codigo em vez de ser digitado aqui.

# A catraca

So DESCE, e nasce hoje em 0: depois de `74de67e` nenhuma struct de `crates/*/src`
deriva `Debug` com segredo dentro, e nenhum `impl` a mao le o campo. Um numero
maior reprova nomeando struct, campo, tipo e o caminho pelo qual vaza.

E o `--catraca` roda o autoteste ANTES de medir: regua que perdeu uma das tres
partes mede zero com o defeito reposto, e um zero desses e pior que o defeito.
"""
import glob
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

TETO_DEBUG_COM_SEGREDO = 0

# Parte 1 -- o lexico. Copiado da varredura que achou as nove (§16 do
# SEGURANCA.md traz o comeco dele com reticencias; este e o inteiro). Casa
# como PREFIXO do nome ou de um componente (`senha_do_rele`, `token_remoto`,
# `usa_senha`), e o resto do crivo mata o que sobrar. Uma diferenca da
# varredura de origem: `credencia` no lugar de `credencial`, porque o plural e
# `credenciais` e o prefixo inteiro nao o casava -- o autoteste foi quem disse.
LEXICO = ("senha", "token", "chave", "segredo", "credencia", "privada",
          "password", "secret", "api_key", "apikey", "hash", "pbkdf", "hmac",
          "salt", "nonce", "cookie", "sessao", "autenticacao")

# Ainda a parte 1: o nome que casa o lexico e NEGA o segredo por si. So entra
# aqui o que foi medido no fonte -- `_env` (senha_env, token_env,
# chave_privada_env) e `public` (chave_publica do usuario e do x509). O que a
# varredura temporaria tinha a mais (`_hex`, `_arquivo`, `_path`, `_algoritmo`,
# `_iteracoes`) saiu: `_hex` estava ERRADO (chave em hexadecimal continua sendo
# a chave) e os outros nao casavam campo nenhum -- negador sem caso medido e
# furo esperando nome.
NEGA_NO_NOME = (
    ("_env", "e o NOME da variavel de ambiente, nao o valor"),
    ("public", "chave PUBLICA e para ser vista"),
)

# Parte 3 -- os falsos positivos, declarados. `(struct, campo, motivo)`; a
# struct `*` casa qualquer uma, e so existe para o `chave_do_fio`, porque ali
# o motivo e do NOME (e o pino, em toda struct que o carrega). Uma entrada que
# nao casa campo nenhum e MORTA e reprova: chave morta e pior que chave
# faltando, porque parece que protege.
ISENTOS = (
    ("Botao", "chave",
     "seletor de CSS pelo qual a bateria alcanca o botao (conferidor_botoes.rs)"),
    ("Sincronia", "chave",
     "NOME da coluna de chave primaria, igual nos dois lados (dblink/sincronia.rs)"),
    ("Violacao", "chave",
     "NOME da chave estrangeira violada, para o relatorio (integridade.rs)"),
    ("PassoAoAlterar", "chave",
     "NOME da chave, guardado para o recado do erro (table.rs)"),
    ("PapelDeChave", "chaves_estrangeiras",
     "NOMES das chaves estrangeiras em que a coluna entra (schema.rs)"),
    ("Transacao", "chaves",
     "VALORES de chave unica ja empilhados por `tabela|indice`, para a "
     "conferencia de unicidade dentro da transacao -- dado, nao credencial "
     "(transacao.rs)"),
    ("*", "chave_do_fio",
     "o PINO: a chave PUBLICA esperada do outro lado, em hexadecimal -- "
     "ve-la e o que diagnostica pino torto (config.rs, odbc/conexao.rs)"),
    ("ParadaDaTabela", "chave_da_posicao",
     "chave de MAPA -- `origem|db/tab`, o indice de `posicoes_bidi` no "
     "servidor. Guardada em vez de remontada para nao haver duas receitas "
     "da mesma chave; ve-la e o que diagnostica par parado (bidirecional.rs)"),
    ("Cru", "chave",
     "chave de MENSAGEM da fabrica de idiomas -- o que o relatorio do "
     "conferidor precisa imprimir para dizer QUAL texto esta cru. Esconde-la "
     "deixaria o achado sem endereco (conferidor_texto_cru.rs)"),
)


# ---------------------------------------------------------------------------
# Lexico do Rust, o minimo: apagar comentario e miolo de literal, preservando
# posicao e linha. Sem isto, um `#[derive(Debug)]` num exemplo de doc-comment
# ou um `"struct X {"` numa string contam como codigo.
# ---------------------------------------------------------------------------

RE_CRU = re.compile(r'b?r(#*)"')
RE_CHAR = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")


def despir(texto):
    saida = list(texto)
    n = len(texto)

    def apagar(a, b):
        for k in range(a, b):
            if saida[k] != "\n":
                saida[k] = " "

    i = 0
    while i < n:
        c = texto[i]
        d = texto[i + 1] if i + 1 < n else ""
        if c == "/" and d == "/":
            j = texto.find("\n", i)
            j = n if j < 0 else j
            apagar(i, j)
            i = j
            continue
        if c == "/" and d == "*":
            prof, j = 1, i + 2
            while j < n and prof:
                if texto.startswith("/*", j):
                    prof, j = prof + 1, j + 2
                elif texto.startswith("*/", j):
                    prof, j = prof - 1, j + 2
                else:
                    j += 1
            apagar(i, j)
            i = j
            continue
        palavra_antes = i > 0 and (texto[i - 1].isalnum() or texto[i - 1] == "_")
        m = None if palavra_antes else RE_CRU.match(texto, i)
        if m:
            fecho = '"' + m.group(1)
            j = texto.find(fecho, m.end())
            j = n if j < 0 else j
            apagar(m.end(), j)
            i = j + len(fecho)
            continue
        if c == '"' or (c == "b" and d == '"' and not palavra_antes):
            ini = i + (2 if c == "b" else 1)
            j = ini
            while j < n and texto[j] != '"':
                j += 2 if texto[j] == "\\" else 1
            apagar(ini, j)
            i = j + 1
            continue
        if c == "'":
            m = RE_CHAR.match(texto, i)
            if m:
                apagar(i + 1, m.end() - 1)
                i = m.end()
                continue
        i += 1
    return "".join(saida)


PARES = {"(": ")", "[": "]", "{": "}", "<": ">"}


def fechar(texto, i, abre):
    """Indice logo DEPOIS do fecho que casa `texto[i] == abre`."""
    fecha, prof, n = PARES[abre], 0, len(texto)
    while i < n:
        c = texto[i]
        if abre == "<" and c == "-" and texto.startswith("->", i):
            i += 2
            continue
        if c == abre:
            prof += 1
        elif c == fecha:
            prof -= 1
            if prof == 0:
                return i + 1
        i += 1
    return n


def pedacos_no_topo(texto, a, b):
    """Os trechos `[a, b)` separados por virgula fora de qualquer par."""
    saida, prof, ini, i = [], 0, a, a
    while i < b:
        c = texto[i]
        if c == "-" and texto.startswith("->", i):
            i += 2
            continue
        if c in "([{<":
            prof += 1
        elif c in ")]}>":
            prof -= 1
        elif c == "," and prof == 0:
            saida.append((ini, i))
            ini = i + 1
        i += 1
    if texto[ini:b].strip():
        saida.append((ini, b))
    return saida


def pular_brancos(texto, i):
    while i < len(texto) and texto[i].isspace():
        i += 1
    return i


def pular_atributos(texto, i):
    i = pular_brancos(texto, i)
    while texto.startswith("#[", i) or texto.startswith("#![", i):
        i = fechar(texto, texto.index("[", i), "[")
        i = pular_brancos(texto, i)
    return i


RE_PUB = re.compile(r"pub(?:\s*\([^)]*\))?\s*")


def pular_pub(texto, i):
    m = RE_PUB.match(texto, i)
    return pular_brancos(texto, m.end()) if m else i


RE_PUB_ATRAS = re.compile(r"pub(?:\s*\([^)]*\))?\s*\Z")


def inicio_do_item(texto, pos):
    """De `struct`/`enum` para tras, passando o `pub(...)` -- os atributos ficam antes dele."""
    m = RE_PUB_ATRAS.search(texto, max(0, pos - 80), pos)
    return m.start() if m else pos


def atributos_acima(texto, pos):
    """Os `#[...]` colados acima de `pos`, inclusive os que atravessam linhas."""
    atrs, j = [], pos
    while True:
        k = j - 1
        while k >= 0 and texto[k].isspace():
            k -= 1
        if k < 0 or texto[k] != "]":
            return atrs
        prof, m = 0, k
        while m >= 0:
            if texto[m] == "]":
                prof += 1
            elif texto[m] == "[":
                prof -= 1
                if prof == 0:
                    break
            m -= 1
        if m <= 0 or texto[m - 1] != "#":
            return atrs
        atrs.append(texto[m - 1:k + 1])
        j = m - 1


RE_DERIVE = re.compile(r"\bderive\s*\(")


def deriva_debug(atributos):
    for a in atributos:
        for m in RE_DERIVE.finditer(a):
            fim = fechar(a, m.end() - 1, "(")
            if re.search(r"\bDebug\b", a[m.end():fim - 1]):
                return True
    return False


# ---------------------------------------------------------------------------
# Os itens: struct (com nome, de tupla, unidade) e enum (variantes com campo).
# ---------------------------------------------------------------------------

RE_ITEM = re.compile(r"\b(struct|enum)\s+([A-Za-z_]\w*)")
RE_IMPL_DEBUG = re.compile(
    r"\bimpl\s*(?:<[^>]*>)?\s*(?:[\w:]+::)?Debug\s+for\s+([A-Za-z_]\w*)")
RE_CAMPO = re.compile(r"([A-Za-z_]\w*)\s*:\s*", re.S)


def linha_de(texto, pos):
    return texto.count("\n", 0, pos) + 1


def campos_nomeados(texto, a, b, prefixo=""):
    """`nome: Tipo` de cada pedaco entre `a` e `b` -- o tipo pode atravessar linhas."""
    campos = []
    for ini, fim in pedacos_no_topo(texto, a, b):
        i = pular_pub(texto, pular_atributos(texto, ini))
        m = RE_CAMPO.match(texto, i, fim)
        if not m:
            continue
        tipo = " ".join(texto[m.end():fim].split())
        campos.append((prefixo + m.group(1), tipo, linha_de(texto, i)))
    return campos


def campos_de_tupla(texto, a, b, dono):
    """Struct de tupla nao nomeia o campo: o lexico se aplica ao NOME da struct."""
    campos = []
    for k, (ini, fim) in enumerate(pedacos_no_topo(texto, a, b)):
        i = pular_pub(texto, pular_atributos(texto, ini))
        tipo = " ".join(texto[i:fim].split())
        if tipo:
            campos.append((f"{dono}.{k}", tipo, linha_de(texto, i)))
    return campos


def campos_de_enum(texto, a, b):
    campos = []
    for ini, fim in pedacos_no_topo(texto, a, b):
        i = pular_atributos(texto, ini)
        m = re.compile(r"([A-Za-z_]\w*)").match(texto, i, fim)
        if not m:
            continue
        variante = m.group(1)
        j = pular_brancos(texto, m.end())
        if j < fim and texto[j] == "{":
            f = fechar(texto, j, "{")
            campos += campos_nomeados(texto, j + 1, f - 1, prefixo=variante + "::")
        elif j < fim and texto[j] == "(":
            f = fechar(texto, j, "(")
            campos += campos_de_tupla(texto, j + 1, f - 1, variante)
    return campos


def corpo_do_item(texto, pos):
    """Depois do nome: pula genericos e `where`; devolve (forma, abre, fecha)."""
    i = pular_brancos(texto, pos)
    if texto.startswith("<", i):
        i = pular_brancos(texto, fechar(texto, i, "<"))
    if texto.startswith("(", i):
        return "tupla", i, fechar(texto, i, "(")
    if texto.startswith("where", i):
        j = texto.find("{", i)
        k = texto.find(";", i)
        if j < 0 or (0 <= k < j):
            return "unidade", i, i
        return "chaves", j, fechar(texto, j, "{")
    if texto.startswith("{", i):
        return "chaves", i, fechar(texto, i, "{")
    return "unidade", i, i


class Item:
    def __init__(self, arquivo, nome, especie, linha, deriva, campos):
        self.arquivo, self.nome, self.especie = arquivo, nome, especie
        self.linha, self.deriva, self.campos = linha, deriva, campos
        self.impl = None  # corpo do `impl Debug` a mao, se houver


def itens_do_arquivo(arquivo, texto):
    limpo = despir(texto)
    itens, impls = [], []
    for m in RE_ITEM.finditer(limpo):
        especie, nome = m.group(1), m.group(2)
        atrs = atributos_acima(limpo, inicio_do_item(limpo, m.start()))
        forma, a, b = corpo_do_item(limpo, m.end())
        if especie == "enum":
            campos = campos_de_enum(limpo, a + 1, b - 1) if forma == "chaves" else []
        elif forma == "chaves":
            campos = campos_nomeados(limpo, a + 1, b - 1)
        elif forma == "tupla":
            campos = campos_de_tupla(limpo, a + 1, b - 1, nome)
        else:
            campos = []
        itens.append(Item(arquivo, nome, especie, linha_de(limpo, m.start()),
                          deriva_debug(atrs), campos))
    for m in RE_IMPL_DEBUG.finditer(limpo):
        i = limpo.find("{", m.end())
        if i < 0:
            continue
        impls.append((m.group(1), limpo[i:fechar(limpo, i, "{")]))
    return itens, impls


# ---------------------------------------------------------------------------
# O crivo.
# ---------------------------------------------------------------------------

def nome_de_segredo(nome):
    """Parte 1: casa o lexico e nenhum negador. Struct de tupla vem `Dono.0`."""
    baixo = nome.lower().split("::")[-1].split(".")[0]
    if not any(baixo.startswith(p) or f"_{p}" in baixo for p in LEXICO):
        return False
    return not any(neg in baixo for neg, _ in NEGA_NO_NOME)


RE_REF = re.compile(r"^&\s*(?:'\w+\s+)?(?:mut\s+)?(.*)$", re.S)
RE_CAMINHO_STD = re.compile(r"\b(?:std|core|alloc)::(?:\w+::)*")
RE_ENVOLTORIO = re.compile(
    r"^(Option|Box|Arc|Rc|Vec|VecDeque|HashSet|BTreeSet|Mutex|RwLock|RefCell|Cell|Cow)\s*<(.*)>$",
    re.S)
RE_MAPA = re.compile(r"^(HashMap|BTreeMap)\s*<(.*)>$", re.S)
LISTAS = ("Vec", "VecDeque", "HashSet", "BTreeSet")


def portador(tipo, em_lista=False, extras=frozenset()):
    """Parte 2: o tipo carrega o valor -- texto ou bytes, direto ou envolto --
    ou um TIPO PORTADOR DESCOBERTO (`extras`, pedido 477): um nome como
    `Segredo` que nao e String nem bytes por ESTRUTURA, mas que algum dono, em
    outro lugar, provou carregar segredo ao redigi-lo no proprio `impl Debug`
    (`tipos_portadores_descobertos()`, mais abaixo)."""
    t = RE_CAMINHO_STD.sub("", " ".join(tipo.split())).strip()
    m = RE_REF.match(t)
    if m:
        return portador(m.group(1), em_lista, extras)
    if t in extras:
        return True
    if t in ("String", "str"):
        return True
    if re.match(r"^\[\s*u8\s*[;\]]", t):
        return True
    if t == "u8":
        return em_lista
    m = RE_ENVOLTORIO.match(t)
    if m:
        # `Option<\n String,\n>` -- o rustfmt deixa virgula final no generico
        dentro = m.group(2).strip().rstrip(",").strip()
        if m.group(1) == "Cow":
            dentro = re.sub(r"^'\w+\s*,\s*", "", dentro)
        return portador(dentro, em_lista or m.group(1) in LISTAS, extras)
    m = RE_MAPA.match(t)
    if m:
        partes = pedacos_no_topo(m.group(2), 0, len(m.group(2)))
        if len(partes) >= 2:
            a, b = partes[1]
            return portador(m.group(2)[a:b], True, extras)
    return False


def desembrulhar(tipo, em_lista=False):
    """O mesmo caminho de `portador()`, mas devolve o NOME de dentro em vez de
    um booleano -- e o que a descoberta de tipos portadores usa para achar
    `Segredo` dentro de `Option<Segredo>`, e para saber que `outro: Definicao`
    NAO e primitivo mas tambem nao interessa (nao aparece redigido em lugar
    nenhum, entao nunca entra no conjunto)."""
    t = RE_CAMINHO_STD.sub("", " ".join(tipo.split())).strip()
    m = RE_REF.match(t)
    if m:
        return desembrulhar(m.group(1), em_lista)
    m = RE_ENVOLTORIO.match(t)
    if m:
        dentro = m.group(2).strip().rstrip(",").strip()
        if m.group(1) == "Cow":
            dentro = re.sub(r"^'\w+\s*,\s*", "", dentro)
        return desembrulhar(dentro, em_lista or m.group(1) in LISTAS)
    m = RE_MAPA.match(t)
    if m:
        partes = pedacos_no_topo(m.group(2), 0, len(m.group(2)))
        if len(partes) >= 2:
            a, b = partes[1]
            return desembrulhar(m.group(2)[a:b], True)
    return t


RE_IDENTIFICADOR_SIMPLES = re.compile(r"^[A-Za-z_]\w*$")


def eh_tipo_de_base(nome):
    """Os tipos que `portador()` ja reconhece por ESTRUTURA -- descobrir de
    novo so acrescentaria ruido ao conjunto (e `String` nunca e um tipo
    "portador descoberto": e portador direto, sempre foi)."""
    return nome in ("String", "str", "u8") or bool(re.match(r"^\[\s*u8\s*[;\]]", nome))


def tipos_portadores_descobertos(itens):
    """Descobre tipos portadores pelo `impl Debug` a mao de QUEM OS USA --
    nunca pelo proprio tipo, e nunca por uma lista digitada (pedido 477).

    Um campo cujo NOME casa o lexico e cujo `impl` a mao NAO o le cru (nem
    substituido por literal, `.field("senha", &"(oculta)")`, nem descartado
    com `campo: _`) prova, de FORA, que o TIPO do campo carrega o segredo
    sozinho -- e essa prova sobrevive a mutacao de UM `impl` isolado, porque
    normalmente ha mais de um dono (e o que faz `Segredo` continuar descoberto
    mesmo quando SO o `impl Debug for Segredo` quebra: `Cifra`, `Email`,
    `CifraFio`, `Definicao` e `CifraDoDblink` continuam provando).

    Devolve (conjunto de nomes, evidencia por nome) -- a evidencia alimenta o
    `--inventario`, para a lista nunca ficar invisivel."""
    descobertos, evidencia = set(), {}
    for it in itens:
        if it.impl is None:
            continue
        for campo, tipo, linha in it.campos:
            if not nome_de_segredo(campo):
                continue
            base = desembrulhar(tipo)
            if eh_tipo_de_base(base) or not RE_IDENTIFICADOR_SIMPLES.match(base):
                continue
            if le_o_campo(it.impl, campo):
                continue
            descobertos.add(base)
            evidencia.setdefault(base, (it.arquivo, linha, it.nome, campo))
    return descobertos, evidencia


def isencao(isentos, item, campo):
    """Parte 3: a leitura declarada. Devolve a entrada que casa, ou None."""
    for ent in isentos:
        if ent[1] == campo.split("::")[-1] and ent[0] in ("*", item.nome):
            return ent
    return None


def le_o_campo(corpo, campo):
    """O `impl` a mao chega ao identificador nu? `senha: _` (descartado) nao conta."""
    nome = campo.split("::")[-1]
    if "." in nome:  # struct de tupla: `self.0`
        return re.search(r"\.\s*%s\b" % nome.split(".")[1], corpo) is not None
    return re.search(r"\b%s\b(?!\s*:\s*_)" % re.escape(nome), corpo) is not None


def le_o_valor_do_campo(corpo, campo):
    """A leitura de um campo de TIPO PORTADOR DESCOBERTO (`Segredo`) e outra
    pergunta: passar o valor INTEIRO adiante e SEGURO, porque quem o imprime
    dali e o `Debug` PROPRIO do tipo -- a mesma camada que protege em toda
    parte (`.field("segredo", segredo)`, em `CifraDoDblink`, nao vaza nada).
    O que fura essa camada e ir ALEM do valor bruto -- um metodo ou campo
    acessado com `.` logo depois do identificador (`self.senha.valor()`, a
    troca de `debug-da-cifra-mostra-a-senha`). Sem esta distincao, todo
    passa-adiante seria um falso positivo."""
    nome = campo.split("::")[-1]
    if "." in nome:  # struct de tupla: `self.0.metodo()`
        alvo = nome.split(".")[1]
        return re.search(r"\.\s*%s\s*\.\s*[A-Za-z_]" % re.escape(alvo), corpo) is not None
    return re.search(r"\b%s\b\s*\.\s*[A-Za-z_]" % re.escape(nome), corpo) is not None


class Resultado:
    def __init__(self):
        self.vazam = []       # (arquivo, linha, item, campo, tipo, via)
        self.isentos = []     # (arquivo, linha, item, campo, entrada)
        self.sem_saida = []   # (arquivo, linha, item, campo, tipo, porque)
        self.notas = []
        self.tipos_descobertos = []   # nomes, ordenados
        self.evidencia_dos_tipos = {}  # nome -> (arquivo, linha, dono, campo)


def analisar(arquivos, isentos=ISENTOS):
    itens, impls = [], []
    for arquivo, texto in arquivos:
        i, p = itens_do_arquivo(arquivo, texto)
        itens += i
        impls += [(arquivo, alvo, corpo) for alvo, corpo in p]
    por_nome = {}
    for it in itens:
        por_nome.setdefault(it.nome, []).append(it)
    r = Resultado()
    for arquivo, alvo, corpo in impls:
        donos = [it for it in por_nome.get(alvo, []) if it.arquivo == arquivo] \
            or por_nome.get(alvo, [])
        if len(donos) != 1:
            r.notas.append(f"{arquivo}: `impl Debug for {alvo}` sem struct unica "
                           f"a que se aplicar ({len(donos)} achadas)")
            continue
        donos[0].impl = corpo

    descobertos, evidencia = tipos_portadores_descobertos(itens)
    r.tipos_descobertos = sorted(descobertos)
    r.evidencia_dos_tipos = evidencia

    usadas = {}
    for it in itens:
        dono_e_portador = it.nome in descobertos
        for campo, tipo, linha in it.campos:
            if not portador(tipo, extras=descobertos):
                continue
            # o NOME do campo casa o lexico (parte 1 de sempre), OU o DONO
            # e ele proprio um tipo portador descoberto (`Segredo.valor` conta
            # mesmo `valor` nao casando nada, porque a struct INTEIRA so
            # existe para carregar o segredo), OU o TIPO do campo e um tipo
            # portador descoberto (`outro: Segredo` conta mesmo `outro` nao
            # casando nada -- pedido 477).
            tipo_base = desembrulhar(tipo)
            if not (nome_de_segredo(campo) or dono_e_portador
                    or tipo_base in descobertos):
                continue
            ent = isencao(isentos, it, campo)
            if ent:
                r.isentos.append((it.arquivo, linha, it, campo, ent))
                usadas.setdefault(ent, []).append(it)
                continue
            if it.deriva:
                r.vazam.append((it.arquivo, linha, it, campo, tipo, "derive(Debug)"))
                continue
            if it.impl is None:
                r.sem_saida.append((it.arquivo, linha, it, campo, tipo,
                                    f"{it.especie} sem Debug"))
                continue
            # campo de tipo portador descoberto: passar o valor INTEIRO
            # adiante e seguro (o `Debug` do proprio tipo protege); so ir
            # ALEM dele (`self.senha.valor()`) vaza. Campo de tipo primitivo
            # (String, bytes) continua na regra de sempre: qualquer mencao
            # crua vaza.
            if tipo_base in descobertos:
                lido = le_o_valor_do_campo(it.impl, campo)
                via = "impl Debug a mao EXTRAI o valor do tipo portador"
            else:
                lido = le_o_campo(it.impl, campo)
                via = "impl Debug a mao LE o campo"
            if lido:
                r.vazam.append((it.arquivo, linha, it, campo, tipo, via))
            else:
                r.sem_saida.append((it.arquivo, linha, it, campo, tipo,
                                    "impl Debug a mao o esconde"))
    r.estado_das_isencoes = []
    for ent in isentos:
        donos = usadas.get(ent, [])
        if not donos:
            estado = "MORTA"
        elif ent[0] != "*" and len({d.arquivo for d in donos}) > 1:
            estado = "AMBIGUA"
        elif any(d.deriva or d.impl is not None for d in donos):
            estado = "viva"
        else:
            estado = "dormente"
        r.estado_das_isencoes.append((ent, estado, donos))
    return r


def ler_arvore(raiz):
    padrao = os.path.join(raiz, "crates", "*", "src", "**", "*.rs")
    saida = []
    for arq in sorted(glob.glob(padrao, recursive=True)):
        with open(arq, encoding="utf-8", errors="replace") as f:
            saida.append((os.path.relpath(arq, raiz), f.read()))
    return saida


def raiz_pedida():
    if "--raiz" in sys.argv:
        return os.path.abspath(sys.argv[sys.argv.index("--raiz") + 1])
    return RAIZ


def medido(r):
    return len(r.vazam)


# ---------------------------------------------------------------------------
# As saidas.
# ---------------------------------------------------------------------------

def imprimir_vazamentos(r):
    for arquivo, linha, it, campo, tipo, via in r.vazam:
        print(f"   VAZA  {arquivo}:{linha}  {it.nome}.{campo}: {tipo}  ({via})")


def problemas_nas_isencoes(r):
    ruim = []
    for ent, estado, donos in r.estado_das_isencoes:
        if estado == "MORTA":
            ruim.append(f"isencao MORTA  {ent[0]}.{ent[1]} -- nenhum campo com "
                        "esse nome casa o crivo; tire a linha de ISENTOS")
        elif estado == "AMBIGUA":
            ruim.append(f"isencao AMBIGUA  {ent[0]}.{ent[1]} -- casa em "
                        + ", ".join(sorted({d.arquivo for d in donos}))
                        + "; o motivo foi lido para UMA delas")
    return ruim


def catraca():
    print("=== a catraca do `Debug` com segredo dentro ===")
    falhas = autoteste(silencioso=True)
    if falhas:
        print("   AUTOTESTE FALHOU: " + ", ".join(falhas))
        print("   Reprovado: a regua perdeu uma parte do crivo; o numero abaixo "
              "nao vale.")
        return 1
    print(f"   autoteste: {N_CASOS} casos ok")
    r = analisar(ler_arvore(raiz_pedida()))
    vivas = sum(1 for _, e, _ in r.estado_das_isencoes if e == "viva")
    dorm = [f"{ent[0]}.{ent[1]}" for ent, e, _ in r.estado_das_isencoes if e == "dormente"]
    print(f"   isencoes declaradas: {len(r.estado_das_isencoes)} -- {vivas} vivas"
          + (f", {len(dorm)} dormente(s) ({', '.join(dorm)}: sem Debug hoje)"
             if dorm else ""))
    print(f"   tipos portadores descobertos pelo impl alheio: "
          f"{len(r.tipos_descobertos)}"
          + (f" ({', '.join(r.tipos_descobertos)})" if r.tipos_descobertos else ""))
    for n in r.notas:
        print("   nota  " + n)
    imprimir_vazamentos(r)
    ruim = problemas_nas_isencoes(r)
    for p in ruim:
        print("   " + p)
    m = medido(r)
    if m > TETO_DEBUG_COM_SEGREDO:
        print(f"\n   SUBIU  {m} (teto {TETO_DEBUG_COM_SEGREDO})")
        print("   Reprovado: segredo no `Debug` vaza no dia em que alguem "
              "acrescentar um `dbg!`. Escreva o `impl Debug` a mao, "
              "desestruturando SEM `..` (molde: `config.rs`, `Cifra`).")
        return 1
    if m < TETO_DEBUG_COM_SEGREDO:
        print(f"\n   DESCEU -- BAIXE O TETO  {m} (teto {TETO_DEBUG_COM_SEGREDO})")
        print("   Ponha o numero novo em TETO_DEBUG_COM_SEGREDO, no mesmo "
              "commit -- catraca frouxa nao segura nada.")
        return 1
    if ruim:
        print(f"\n   {m} (teto {TETO_DEBUG_COM_SEGREDO}), mas a lista de "
              "isencoes envelheceu -- reprovado")
        return 1
    print(f"\n   ok  {m} (teto {TETO_DEBUG_COM_SEGREDO})")
    return 0


def numeros():
    """Saida de maquina para o `docs/qa/medir.py`: ele NAO le a prosa acima.

    O autoteste roda aqui tambem, e por um motivo medido: com a deteccao do
    `derive` mutada, a arvore limpa continua medindo 0 -- o zero da regua
    morta e igual ao zero da arvore sa. Um `--numeros` que nao se provasse
    entregaria esse zero ao inventario, que o publicaria como «em cima, sem
    folga». Sair com erro faz o inventario dizer «nao rodou», que e a verdade."""
    falhas = autoteste(silencioso=True)
    if falhas:
        print("autoteste da regua falhou: " + ", ".join(falhas), file=sys.stderr)
        return 1
    r = analisar(ler_arvore(raiz_pedida()))
    print(f"catraca:nome=TETO_DEBUG_COM_SEGREDO;onde={EU};"
          f"valor={TETO_DEBUG_COM_SEGREDO};medido={medido(r)};tipo=teto;"
          "mede=campos de segredo que o `Debug` de uma struct de crates/*/src "
          "imprime, derivado ou a mao")
    return 0


def inventario():
    r = analisar(ler_arvore(raiz_pedida()))
    print("=== campos que passam pelo nome e pelo tipo, em tres grupos ===\n")
    print(f"-- TIPOS PORTADORES DESCOBERTOS ({len(r.tipos_descobertos)}): nao "
          "sao String/bytes por estrutura, mas algum dono os redige")
    for nome in r.tipos_descobertos:
        arquivo, linha, dono, campo = r.evidencia_dos_tipos[nome]
        print(f"   {nome}  -- provado por {arquivo}:{linha}  {dono}.{campo}")
    print(f"\n-- CONTAM ({len(r.vazam)}): o Debug os imprime")
    imprimir_vazamentos(r)
    print(f"\n-- ISENTOS ({len(r.isentos)}): lidos, e nao sao segredo")
    for arquivo, linha, it, campo, ent in r.isentos:
        print(f"   {arquivo}:{linha}  {it.nome}.{campo}  -- {ent[2]}")
    print(f"\n-- NAO VAZAM POR ESTA SAIDA ({len(r.sem_saida)})")
    for arquivo, linha, it, campo, tipo, porque in r.sem_saida:
        print(f"   {arquivo}:{linha}  {it.nome}.{campo}: {tipo}  ({porque})")
    print("\n-- estado das isencoes")
    for ent, estado, _ in r.estado_das_isencoes:
        print(f"   {estado:9} {ent[0]}.{ent[1]}")
    for n in r.notas:
        print("   nota  " + n)
    return 0


# ---------------------------------------------------------------------------
# O autoteste: cada parte do crivo, nos dois sentidos, no MESMO `analisar` que
# le a arvore. Mutar uma parte tem de derrubar pelo menos um caso.
# ---------------------------------------------------------------------------

CASOS = []


def caso(nome, texto, vazam, isentos=(), extra=None):
    CASOS.append((nome, texto, vazam, isentos, extra))


caso("struct com derive e sem segredo: zero",
     "#[derive(Debug, Clone)]\npub struct A { pub nome: String, pub porta: u16 }\n",
     [])
caso("derive(Debug) com `senha: String` conta",
     "#[derive(Debug)]\nstruct A { senha: String }\n",
     ["A.senha"])
caso("`senha_env: String` nao conta (nome nega)",
     "#[derive(Debug)]\nstruct A { senha_env: String }\n",
     [])
caso("`chave_publica: [u8; 32]` nao conta (nome nega)",
     "#[derive(Debug)]\nstruct A { chave_publica: [u8; 32] }\n",
     [])
caso("`usa_senha: bool` nao conta (tipo)",
     "#[derive(Debug)]\nstruct A { usa_senha: bool, token_id: u64 }\n",
     [])
caso("sem derive e sem impl: nao vaza por esta saida",
     "struct A { senha: String }\n",
     [])
caso("derive em varias linhas conta",
     "#[derive(\n    Clone,\n    Debug,\n)]\npub(crate) struct A {\n    pub token: Option<String>,\n}\n",
     ["A.token"])
caso("`cfg_attr(test, derive(Debug))` conta",
     "#[cfg_attr(test, derive(Debug))]\nstruct A { segredo: Vec<u8> }\n",
     ["A.segredo"])
caso("derive num comentario, numa doc e numa string nao conta",
     "/// ```\n/// #[derive(Debug)]\n/// ```\n// #[derive(Debug)]\n"
     "const X: &str = \"#[derive(Debug)]\";\nstruct A { senha: String }\n",
     [])
caso("`derive(MeuDebug)` nao e Debug",
     "#[derive(MeuDebug)]\nstruct A { senha: String }\n",
     [])
caso("tipo que atravessa linhas ainda e lido",
     "#[derive(Debug)]\nstruct A {\n    credencial:\n        Option<\n            String,\n        >,\n}\n",
     ["A.credencial"])
caso("envoltorios: Vec<String>, &str, Box<str>, Cow<str>, HashMap<_, String>, Option<Vec<u8>>",
     "#[derive(Debug)]\nstruct A<'a> { tokens: Vec<String>, senha: &'a str, "
     "chave: Box<str>, segredo: std::borrow::Cow<'a, str>, "
     "credenciais: HashMap<String, String>, salt: Option<Vec<u8>> }\n",
     ["A.tokens", "A.senha", "A.chave", "A.segredo", "A.credenciais", "A.salt"])
caso("tipos que nao carregam valor: u32, Option<Outro>, Vec<Outro>",
     "#[derive(Debug)]\nstruct A { hash: u32, token: Option<Tipo>, chaves: Vec<Coluna> }\n",
     [])
caso("impl a mao que LE o campo conta (`.field(\"senha\", &self.senha)`)",
     "struct A { senha: String, senha_env: String }\n"
     "impl std::fmt::Debug for A {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n"
     "        f.debug_struct(\"A\").field(\"senha\", &self.senha).field(\"senha_env\", &self.senha_env).finish()\n    }\n}\n",
     ["A.senha"])
caso("impl a mao que descarta (`senha: _`) e escreve literal nao conta",
     "struct A { senha: String, token: String }\n"
     "impl fmt::Debug for A {\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n"
     "        let A { senha: _, token: _ } = self;\n"
     "        f.debug_struct(\"A\").field(\"senha\", &\"(oculta)\").field(\"token\", &\"(oculto)\").finish()\n    }\n}\n",
     [])
caso("a troca da guarda: desestrutura e imprime (`senha,` + `.field(\"senha\", senha)`)",
     "struct A { senha: String, token: String }\n"
     "impl Debug for A {\n    fn fmt(&self, f: &mut Formatter<'_>) -> Result {\n"
     "        let A { senha, token: _ } = self;\n"
     "        f.debug_struct(\"A\").field(\"senha\", senha).field(\"token\", &\"(oculto)\").finish()\n    }\n}\n",
     ["A.senha"])
caso("isento declarado nao conta, e a isencao fica viva",
     "#[derive(Debug)]\nstruct Botao { chave: Option<String> }\n",
     [], isentos=(("Botao", "chave", "seletor"),), extra=("viva",))
caso("isencao por nome (`*`) vale em qualquer struct",
     "#[derive(Debug)]\nstruct A { chave_do_fio: String }\n#[derive(Debug)]\nstruct B { chave_do_fio: String }\n",
     [], isentos=(("*", "chave_do_fio", "pino"),), extra=("viva",))
caso("isencao que nao casa nada e MORTA",
     "#[derive(Debug)]\nstruct A { nome: String }\n",
     [], isentos=(("Sumiu", "chave", "x"),), extra=("MORTA",))
caso("isencao dormente: a struct existe e nao tem Debug",
     "struct PassoAoAlterar { chave: String }\n",
     [], isentos=(("PassoAoAlterar", "chave", "nome"),), extra=("dormente",))
caso("isencao so vale para a struct nomeada",
     "#[derive(Debug)]\nstruct Botao { chave: String }\n#[derive(Debug)]\nstruct Outra { chave: String }\n",
     ["Outra.chave"], isentos=(("Botao", "chave", "seletor"),))
caso("enum: variante com campo nomeado conta",
     "#[derive(Debug)]\nenum Op { Login { login: String, senha: String }, Sair }\n",
     ["Op.Login::senha"])
caso("struct de tupla: o lexico vale para o nome da struct",
     "#[derive(Debug)]\npub struct Token(String);\n#[derive(Debug)]\npub struct Porta(u16);\n",
     ["Token.Token.0"])
caso("struct de tupla com impl que escreve literal nao conta; que le `self.0` conta",
     "struct Chave([u8; 32]);\nimpl std::fmt::Debug for Chave {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(\"Chave(oculta)\") }\n}\n"
     "struct Senha(String);\nimpl std::fmt::Debug for Senha {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, \"{}\", self.0) }\n}\n",
     ["Senha.Senha.0"])
caso("genericos e where nao confundem o corpo",
     "#[derive(Debug)]\nstruct A<T: Fn(u8) -> u8> where T: Clone { f: T, token: String }\n",
     ["A.token"])
caso("`pub(crate)` entre o derive e a struct nao esconde o derive",
     "#[derive(Debug)]\npub(in crate::x) struct A { nonce: [u8; 12] }\n",
     ["A.nonce"])

# --- pedido 477: o tipo portador DESCOBERTO pelo impl alheio, nao cravado ---

caso("tipo portador descoberto pelo impl ALHEIO: quem so PASSA o valor "
     "adiante fica limpo; quem EXTRAI conta",
     "struct Cofre { valor: String }\n"
     "impl std::fmt::Debug for Cofre {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(\"Cofre(oculto)\") }\n}\n"
     "struct Config { senha: Cofre }\n"
     "impl std::fmt::Debug for Config {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.debug_struct(\"Config\").field(\"senha\", &\"(oculta)\").finish()\n    }\n}\n"
     "struct Recado { senha: Cofre }\n"
     "impl std::fmt::Debug for Recado {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        let Recado { senha } = self;\n        f.debug_struct(\"Recado\").field(\"senha\", senha).finish()\n    }\n}\n"
     "struct Extrai { senha: Cofre }\n"
     "impl std::fmt::Debug for Extrai {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        write!(f, \"{}\", self.senha.valor())\n    }\n}\n",
     ["Extrai.senha"])
caso("tipo descoberto: o PROPRIO tipo lendo o campo cru conta -- a troca da "
     "guarda `debug-do-segredo-mostra-o-valor`",
     "struct Cofre2 { valor: String }\n"
     "impl std::fmt::Debug for Cofre2 {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.debug_struct(\"Cofre2\").field(\"valor\", &self.valor).finish()\n    }\n}\n"
     "struct Config2 { senha: Cofre2 }\n"
     "impl std::fmt::Debug for Config2 {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.debug_struct(\"Config2\").field(\"senha\", &\"(oculta)\").finish()\n    }\n}\n",
     ["Cofre2.valor"])
caso("tipo descoberto: campo de tipo portador conta ao DERIVAR, mesmo o nome "
     "do campo nao casando o lexico",
     "struct Cofre3 { valor: String }\n"
     "impl std::fmt::Debug for Cofre3 {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(\"Cofre3(oculto)\") }\n}\n"
     "struct Config3 { senha: Cofre3 }\n"
     "impl std::fmt::Debug for Config3 {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.debug_struct(\"Config3\").field(\"senha\", &\"(oculta)\").finish()\n    }\n}\n"
     "#[derive(Debug)]\nstruct Empacota { guardado: Cofre3 }\n",
     ["Empacota.guardado"])
caso("tipo NAO descoberto se ninguem o redige -- todo mundo so PASSA o campo "
     "adiante, e a regua continua sem ver (limite declarado no cabecalho)",
     "struct Aberto { valor: String }\n"
     "impl std::fmt::Debug for Aberto {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, \"{}\", self.valor) }\n}\n"
     "struct Usa { senha: Aberto }\n"
     "impl std::fmt::Debug for Usa {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        let Usa { senha } = self;\n        f.debug_struct(\"Usa\").field(\"senha\", senha).finish()\n    }\n}\n",
     [])

N_CASOS = len(CASOS)


def autoteste(silencioso=False):
    falhas = []
    for nome, texto, esperado, isentos, extra in CASOS:
        r = analisar([("caso.rs", texto)], isentos=isentos)
        vistos = sorted(f"{it.nome}.{campo}" for _, _, it, campo, _, _ in r.vazam)
        ok = vistos == sorted(esperado)
        detalhe = f"viu {vistos}, esperava {sorted(esperado)}"
        if ok and extra:
            estados = [e for _, e, _ in r.estado_das_isencoes]
            ok = estados == list(extra)
            detalhe = f"isencoes {estados}, esperava {list(extra)}"
        if not silencioso:
            print("   %s  %s%s" % ("ok  " if ok else "FALHOU", nome,
                                   "" if ok else "  -- " + detalhe))
        if not ok:
            falhas.append(nome)
    if not silencioso:
        print("   %s" % ("todos passaram" if not falhas
                         else "FALHOU: " + ", ".join(falhas)))
    return falhas


def principal():
    if "--autoteste" in sys.argv:
        print("=== autoteste da regua do `Debug` com segredo ===")
        return 1 if autoteste() else 0
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    if "--inventario" in sys.argv:
        return inventario()
    r = analisar(ler_arvore(raiz_pedida()))
    imprimir_vazamentos(r)
    for p in problemas_nas_isencoes(r):
        print("   " + p)
    return 0


if __name__ == "__main__":
    sys.exit(principal())
