#!/usr/bin/env python3
"""O MAPA da trava global: quantas secoes criticas existem, e o que cada uma SEGURA.

    python3 bancada/concorrencia/mapa-da-trava.py            # o mapa legivel
    python3 bancada/concorrencia/mapa-da-trava.py --json     # para outro gerador
    python3 bancada/concorrencia/mapa-da-trava.py --classe rede   # so uma classe

Por que este medidor existe
---------------------------
A SP000011 e «remover o `Mutex<Instancia>` global», e a pergunta que decide o
desenho substituto nao e *quantas* tomadas existem: e **o que cada uma segura
enquanto esta com a trava na mao**. Uma secao critica de 3 us e uma de 40 ms
sao dois problemas diferentes, e a CONTAGEM nao os distingue -- foi por contar
sem olhar o conteudo que «as 13 tomadas fora do ponto unico» virou um item que
ficou parado depois de resolvido.

Este e um medidor ESTATICO de proposito: ele le o fonte e nao roda o servidor.
Numero de concorrencia tirado de maquina ocupada e ruido; numero tirado do
fonte vale em qualquer maquina, e e o que se pode entregar com honestidade
enquanto outras frentes compilam ao lado.

O que ele conta, e o que ele NAO conta
--------------------------------------
CONTA: as chamadas a `travar_dados()` fora da definicao e fora dos testes; a
extensao em linhas de cada secao critica (do ponto da tomada ate o fim do bloco
que a segura, ou ate um `drop` explicito); e os marcadores de trabalho caro
alcancaveis de dentro dela.

NAO CONTA: tempo. Este medidor nao diz «40 ms» -- diz «desta secao se alcanca
um `sync_all`», que e uma afirmacao sobre o fonte e nao sobre o relogio.
*Numero citado e numero que nao se mede*: quem quiser o tempo roda o
`a-trava-serializa.py`, que mede efeito.

As tres armadilhas que este medidor ja teve de desarmar
-------------------------------------------------------
1. **Comentario nao e codigo.** Todo comentario e todo literal de texto viram
   espaco antes da varredura -- senao o proprio cabecalho deste arquivo, que
   escreve `sync_all` acima, seria contado como uma tomada de disco. E a mesma
   licao do `so_um_lugar_toma_a_trava`, que acusou 4 onde havia 1 porque a
   agulha estava escrita nele.
2. **Resolucao por NOME sobre-aproxima.** `self.aplicar(` e `t.aplicar(`
   resolvem para o mesmo nome, e duas funcoes homonimas em `impl` diferentes
   viram uma so. Isso ERRA para o lado seguro (acusa trabalho que talvez nao
   aconteca), e por isso o mapa mostra o CAMINHO ate o marcador: quem le
   confere o caminho em vez de acreditar no rotulo.
3. **Profundidade e escolha, nao acidente.** Com profundidade infinita tudo
   alcanca tudo por um ajudante comum, e o mapa vira uma coluna de «caro».
   O padrao e 3 saltos, e o caminho impresso deixa ver onde a conta esticou.
"""
import json
import pathlib
import re
import signal
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
FONTES = [
    RAIZ / "crates/phxsql-server/src",
    RAIZ / "crates/phxsql-store/src",
    RAIZ / "crates/phxsql-core/src",
]
ALVO = RAIZ / "crates/phxsql-server/src/servidor.rs"
SALTOS = 5
# Acima disto o nome deixa de identificar a funcao: `abrir` tem 23 definicoes
# nesta arvore, `nome` tem 35. Caminho do TETO que passa por um desses nao e
# suspeita, e ruido -- e ruido que faz tudo parecer caro esconde o que e caro.
# Abaixo desta confianca o caminho nao entra no mapa: `disco-escrita via
# nome(3/35)` nao e suspeita, e ruido -- e ruido que faz tudo parecer caro
# esconde o que e caro de verdade.
PISO_DE_CONFIANCA = 0.5
# Acima disto o mapa AFIRMA: todas as definicoes de todo nome do caminho
# alcancam o marcador, entao qual delas e a certa deixou de importar.
CERTO = 0.999
# Quantos caminhos alternativos se guardam por classe. Ver `melhores`.
TOP = 4
# Um caminho que e a melhor prova em pelo menos esta fatia das secoes nao
# distingue secao nenhuma: e a PORTA COMUM por onde todas passam. Ele sai da
# classificacao e sobe para o cabecalho, uma vez, com o numero de secoes que o
# herdam. A fatia e medida, e nao uma lista de nomes escrita a mao -- lista
# escrita a mao envelhece calada.
PORTA_COMUM = 0.20

# Os marcadores do trabalho caro. A chave e a classe; o valor, as agulhas.
# Cada uma esta aqui porque responde a UMA pergunta do desenho substituto:
# «esta secao pode ser compartilhada por leitores?» (escrita), «ela para em
# I/O?» (disco), «ela atravessa a rede?» (rede), «ela dura o que o dono do
# banco quiser?» (usuario), «ela percorre a tabela?» (varredura).
MARCADORES = {
    "durabilidade": [r"\bsync_all\b", r"\bsync_data\b"],
    "disco-escrita": [r"\bwrite_all\b", r"\bset_len\b", r"\bFile::create\b",
                      r"\bfs::remove_file\b", r"\bfs::rename\b", r"\bfs::write\b",
                      r"\bcreate_dir_all\b"],
    "disco-leitura": [r"\bread_exact\b", r"\bread_to_end\b", r"\bread_to_string\b",
                      r"\bseek\b", r"\bmetadata\s*\("],
    # OPERACAO de rede, nunca o TIPO. `TcpStream` sozinho estava na AGULHA e
    # aparecia em ASSINATURA: `Cliente::montar(fluxo: TcpStream, ...)` fez o
    # `op_juntar` ser classificado como «atravessa a rede» com confianca 1,0,
    # por homonimo com o `montar` do `pag.rs`. Conferido a mao: `juncao.rs` tem
    # ZERO TcpStream. Marcador que casa com declaracao de tipo mede o
    # vocabulario do arquivo, e nao o que ele faz.
    "rede": [r"TcpStream::connect", r"TcpListener::bind", r"\bset_read_timeout\b",
             r"\bset_write_timeout\b", r"\bpeer_addr\b", r"\.shutdown\s*\("],
    "espera": [r"\bthread::sleep\b", r"\bsleep\s*\("],
    # O corpo de um gatilho BEFORE roda DENTRO da trava (o AFTER nao -- o
    # comentario do `rodar_gatilhos_depois` diz isso com todas as letras). Por
    # isso a agulha e o BEFORE e o interpretador, e nao a palavra «gatilho»:
    # metade das mencoes a gatilho neste arquivo e do caminho que NAO segura.
    "usuario": [r"\brodar_gatilhos_antes\b", r"\brotina::executar\b",
                r"\bContexto::de_gatilho\b"],
}
# A varredura e medida a parte: laco NA PROPRIA secao critica pesa diferente de
# laco tres saltos abaixo, dentro de um ajudante que talvez nem seja chamado.
LACO = re.compile(r"^\s*(for|while|loop)\b|\.iter\(\)|\.iter_mut\(\)|\.into_iter\(\)")

CHAMADA = re.compile(r"\b([a-z_][a-z0-9_]*)\s*\(")
# `let montar = |f, t| ...` e uma FUNCAO LOCAL, e ela nao e a `fn montar` de
# outro arquivo. Sem isto, o `juntar` do `juncao.rs` -- que tem zero operacao
# de rede, conferido a mao -- era classificado «atravessa a rede com a trava na
# mao» com confianca 1,0, porque o nome da closure dele bate com o
# `Cliente::montar(fluxo: TcpStream, ...)` do `replica.rs`. Fechadura local
# some da resolucao do trecho que a declara.
FECHADURA = re.compile(r"\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*=\s*(?:move\s*)?\|")
# Nomes que sao construcao da linguagem ou ruido, e nao funcao nossa.
RUIDO = {
    "if", "while", "for", "match", "return", "fn", "let", "else", "loop",
    "some", "ok", "err", "none", "vec", "format", "println", "eprintln",
    "assert", "assert_eq", "assert_ne", "panic", "write", "writeln", "unwrap",
    "expect", "clone", "to_string", "into", "from", "as_str", "len", "push",
    "map", "and_then", "unwrap_or", "unwrap_or_else", "unwrap_or_default",
    "min", "max", "abs", "trim", "is_empty", "collect", "filter", "iter",
}


def sem_comentario_nem_texto(fonte: str) -> str:
    """Troca comentario e literal de texto por espaco, PRESERVANDO as posicoes.

    Preservar a posicao e o que permite contar chave e achar linha no texto
    limpo e apontar para a linha certa do arquivo original.
    """
    saida = []
    i, n = 0, len(fonte)
    while i < n:
        c = fonte[i]
        # comentario de linha
        if c == "/" and i + 1 < n and fonte[i + 1] == "/":
            j = fonte.find("\n", i)
            j = n if j < 0 else j
            saida.append(" " * (j - i))
            i = j
            continue
        # comentario de bloco, com aninhamento (Rust aninha)
        if c == "/" and i + 1 < n and fonte[i + 1] == "*":
            profundidade, j = 1, i + 2
            while j < n and profundidade:
                if fonte[j] == "/" and j + 1 < n and fonte[j + 1] == "*":
                    profundidade += 1
                    j += 2
                elif fonte[j] == "*" and j + 1 < n and fonte[j + 1] == "/":
                    profundidade -= 1
                    j += 2
                else:
                    j += 1
            saida.append("".join(ch if ch == "\n" else " " for ch in fonte[i:j]))
            i = j
            continue
        # texto cru r"..." / r#"..."#
        if c == "r" and i + 1 < n and fonte[i + 1] in '#"':
            j = i + 1
            cerquilhas = 0
            while j < n and fonte[j] == "#":
                cerquilhas += 1
                j += 1
            if j < n and fonte[j] == '"':
                fecha = '"' + "#" * cerquilhas
                k = fonte.find(fecha, j + 1)
                k = n if k < 0 else k + len(fecha)
                saida.append("".join(ch if ch == "\n" else " " for ch in fonte[i:k]))
                i = k
                continue
        # texto comum
        if c == '"':
            j = i + 1
            while j < n:
                if fonte[j] == "\\":
                    j += 2
                    continue
                if fonte[j] == '"':
                    j += 1
                    break
                j += 1
            saida.append("".join(ch if ch == "\n" else " " for ch in fonte[i:j]))
            i = j
            continue
        # literal de caractere: '{' e '}' contariam como chave
        if c == "'" and i + 2 < n:
            if fonte[i + 1] == "\\":
                k = fonte.find("'", i + 2)
                if 0 <= k <= i + 6:
                    saida.append(" " * (k + 1 - i))
                    i = k + 1
                    continue
            elif fonte[i + 2] == "'":
                saida.append("   ")
                i += 3
                continue
        saida.append(c)
        i += 1
    return "".join(saida)


def fim_do_bloco(limpo: str, abre: int) -> int:
    """Da a posicao do `}` que fecha o `{` em `abre`."""
    profundidade = 0
    for j in range(abre, len(limpo)):
        if limpo[j] == "{":
            profundidade += 1
        elif limpo[j] == "}":
            profundidade -= 1
            if profundidade == 0:
                return j
    return len(limpo) - 1


def indexar_funcoes(arquivos):
    """nome da funcao -> lista de corpos (texto limpo).

    Lista, e nao um so, porque `impl` diferentes tem metodos homonimos: a
    resolucao aqui e por NOME, e ela une os homonimos de proposito. Sobre-
    aproximar e o erro seguro num mapa de secao critica.
    """
    indice = {}
    for caminho in arquivos:
        limpo = sem_comentario_nem_texto(caminho.read_text(encoding="utf-8"))
        for m in re.finditer(r"\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)", limpo):
            nome = m.group(1)
            # a lista de argumentos, para achar o `{` do corpo depois dela
            p = limpo.find("(", m.end())
            if p < 0:
                continue
            profundidade, j = 0, p
            while j < len(limpo):
                if limpo[j] == "(":
                    profundidade += 1
                elif limpo[j] == ")":
                    profundidade -= 1
                    if profundidade == 0:
                        break
                j += 1
            k = j
            while k < len(limpo) and limpo[k] not in "{;":
                k += 1
            if k >= len(limpo) or limpo[k] == ";":
                continue  # declaracao de trait, sem corpo
            fim = fim_do_bloco(limpo, k)
            indice.setdefault(nome, []).append(limpo[k:fim + 1])
    return indice


def marcadores_em(trecho: str):
    achados = set()
    for classe, agulhas in MARCADORES.items():
        for a in agulhas:
            if re.search(a, trecho):
                achados.add(classe)
                break
    return achados


def alcancaveis(trecho, indice, saltos, memo=None):
    """Os marcadores alcancaveis do trecho, cada um com CONFIANCA medida.

    # O problema que esta funcao existe para nao esconder

    A resolucao aqui e por NOME, e nesta arvore ha nome com 23 definicoes
    (`abrir`), 35 (`nome`), 10 (`sincronizar`). Os dois jeitos obvios erram
    para lados opostos, e os dois ja foram tentados aqui:

      * unir todos os homonimos diz que o `op_varrer` grava em disco -- e ele
        nao grava: ele alcanca um homonimo de `executar` que grava;
      * exigir unanimidade perde que o `op_inserir` faz `fsync` com a trava na
        mao, porque `sincronizar_replicada` tambem se chama sincronizar e nao
        faz. E esse e o achado que mais importa neste mapa.

    Nenhum dos dois e medicao: um chuta para cima, o outro para baixo.

    # O que se mede em vez de chutar

    Para cada salto conta-se **quantas das definicoes daquele nome alcancam o
    marcador**, e o caminho carrega a fracao: `sincronizar(9/10)` quer dizer
    nove das dez definicoes de `sincronizar` chegam a um `sync_all`. A
    confianca do caminho e a MENOR fracao dele -- um elo fraco derruba a
    corrente inteira, que e exatamente o que ele deve fazer.

    Confianca 1,0 e afirmacao: nao importa qual homonimo seja, o marcador esta
    la. Abaixo disso e suspeita com endereco, e quem le confere o caminho.
    """
    if memo is None:
        memo = {}
    # Sem guarda de ciclo POR NOME, e de proposito: quem termina a recursao e
    # a profundidade, que decresce sempre. A guarda por nome parecia certa e
    # escondia o achado que mais importa neste mapa: o `Table::sincronizar`
    # chama `ndx.sincronizar`, mesmo NOME, e a guarda cortava ali -- o
    # `op_inserir` aparecia sem `fsync` porque o medidor se recusava a entrar
    # na segunda funcao homonima do caminho.
    # (caminho, confianca) por classe
    achados = {c: [((), 1.0)] for c in marcadores_em(trecho)}
    if saltos <= 0:
        return achados
    locais = set(FECHADURA.findall(trecho))
    chamados = sorted({n for n in CHAMADA.findall(trecho)
                       if n not in RUIDO and n not in locais and n in indice})
    for nome in chamados:
        chave = (nome, saltos)
        if chave not in memo:
            corpos = indice[nome]
            junta = {}
            for corpo in corpos:
                for k, caminhos in alcancaveis(corpo, indice, saltos - 1,
                                               memo).items():
                    quantos, acumulado = junta.get(k, (0, []))
                    junta[k] = (quantos + 1, melhores(acumulado + caminhos))
            saida = {}
            for k, (quantos, caminhos) in junta.items():
                n = len(corpos)
                fracao = quantos / n
                rotulo = nome if fracao == 1.0 and n == 1 else f"{nome}({quantos}/{n})"
                saida[k] = [((rotulo,) + cam, min(fracao, cf))
                            for cam, cf in caminhos]
            memo[chave] = saida
        for k, caminhos in memo[chave].items():
            achados.setdefault(k, [])
            achados[k] = melhores(achados[k] + caminhos)
    return achados


def melhores(caminhos):
    """Os TOP caminhos distintos, do mais confiavel para o menos.

    Guardar so o melhor parecia bastar e nao bastava: quando o melhor caminho
    de uma classe e a PORTA COMUM (o `abrir_travada`, por onde toda operacao
    de tabela passa), tirar a porta comum do mapa tirava a classe junto -- e
    `op_varrer` deixava de mostrar que tambem alcanca `fsync` por conta
    propria. Sem alternativa guardada nao ha o que mostrar no lugar.
    """
    # A chave e a PORTA -- a primeira funcao chamada de dentro da secao --, e
    # nao o caminho inteiro. Guardando por caminho inteiro, `espelhar ->
    # sincronizar`, `espelhar -> sincronizar -> sincronizar` e `espelhar ->
    # espelhar -> sincronizar` ocupavam as tres vagas de alternativa sendo a
    # MESMA porta, e a secao ficava sem alternativa nenhuma para mostrar
    # depois que a porta comum saia.
    visto, saida = set(), []
    for cam, conf in sorted(caminhos, key=lambda x: (-x[1], len(x[0]))):
        porta = porta_de(cam)
        if porta in visto:
            continue
        visto.add(porta)
        saida.append((cam, conf))
        if len(saida) >= TOP:
            break
    return saida


def porta_de(caminho):
    """A primeira funcao chamada de dentro da secao critica -- a porta."""
    return re.sub(r"\(\d+/\d+\)$", "", caminho[0]) if caminho else ""



def classificar(classes, lacos):
    """A classe que decide o desenho substituto, do mais grave para o menos.

    A ordem importa: uma secao que atravessa a rede tambem toca disco, e
    rotula-la «disco» esconderia o que ha de pior nela.
    """
    m = classes
    if "usuario" in m:
        return "codigo-do-dono"
    if "rede" in m or "espera" in m:
        return "rede-ou-espera"
    if "durabilidade" in m:
        return "escrita-duravel"
    if "disco-escrita" in m:
        return "escrita"
    if lacos or "disco-leitura" in m:
        return "leitura-com-varredura"
    return "leitura-curta"


def mapear(alvo=None):
    alvo = alvo or ALVO
    bruto = alvo.read_text(encoding="utf-8")
    limpo = sem_comentario_nem_texto(bruto)
    linha_de = lambda pos: limpo.count("\n", 0, pos) + 1

    # Os modulos de teste, para nao contar o proprio medidor como producao.
    # Sao DEZ neste arquivo e nao um: procurar «mod testes» pegava o primeiro e
    # deixava nove passarem -- e as tomadas dos testes da reentrancia entravam
    # no mapa como se fossem caminho de servidor.
    proibido = []
    for t in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", limpo):
        a = limpo.find("{", t.end())
        if a >= 0:
            proibido.append((t.start(), fim_do_bloco(limpo, a)))
    em_teste = lambda pos: any(a <= pos <= b for a, b in proibido)

    indice = indexar_funcoes(sorted(p for d in FONTES for p in d.rglob("*.rs")))

    # A definicao, para nao contar a si mesma.
    defin = limpo.find("fn travar_dados(&self)")
    memo = {}

    secoes = []
    for m in re.finditer(r"\btravar_dados\s*\(\s*\)", limpo):
        if defin >= 0 and defin <= m.start() <= fim_do_bloco(limpo, limpo.find("{", defin)):
            continue
        if em_teste(m.start()):
            continue
        pos = m.start()
        # A funcao que a contem: o `fn` mais proximo para tras cujo corpo a cobre.
        dona = "?"
        for f in re.finditer(r"\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)", limpo[:pos]):
            dona = f.group(1)
        # O bloco que segura a guarda: sobe ate a `{` aberta mais interna.
        profundidade, abre = 0, None
        for j in range(pos, -1, -1):
            if limpo[j] == "}":
                profundidade += 1
            elif limpo[j] == "{":
                if profundidade == 0:
                    abre = j
                    break
                profundidade -= 1
        fim = fim_do_bloco(limpo, abre) if abre is not None else len(limpo) - 1

        # O nome da guarda, para achar um `drop` explicito que encurte a secao.
        cabeca = limpo[max(0, limpo.rfind("\n", 0, pos)):pos]
        g = re.search(r"let\s+(?:Ok\s*\(\s*)?(?:mut\s+)?([a-zA-Z_][a-zA-Z0-9_]*)", cabeca)
        guarda = g.group(1) if g else "(sem nome)"
        solto = None
        if guarda not in ("(sem nome)",):
            d = re.search(r"\bdrop\s*\(\s*" + re.escape(guarda) + r"\s*\)", limpo[pos:fim])
            if d:
                solto = pos + d.end()
                fim = solto

        secao = limpo[pos:fim]
        alc = alcancaveis(secao, indice, SALTOS, memo)
        secoes.append({
            "linha": linha_de(pos),
            "fim": linha_de(fim),
            "funcao": dona,
            "guarda": guarda,
            "solta_cedo": solto is not None,
            "linhas": linha_de(fim) - linha_de(pos) + 1,
            "lacos_diretos": sum(1 for l in secao.splitlines() if LACO.search(l)),
            "alcanca": {k: [{"via": list(cam), "confianca": round(cf, 3)}
                            for cam, cf in v if cf >= PISO_DE_CONFIANCA]
                        for k, v in alc.items()
                        if any(cf >= PISO_DE_CONFIANCA for _, cf in v)},
        })
    # A PORTA COMUM, medida: caminho que e a melhor prova em quase toda secao
    # nao separa secao nenhuma. `abrir_travada -> espelhar -> sincronizar` e
    # uma: TODA operacao de tabela passa por ela, e enquanto ela contava, 35
    # das 76 secoes viravam «escrita duravel» -- inclusive o `op_ler`. O fato
    # nao se apaga (ele sobe para o cabecalho, com quantas secoes o herdam); o
    # que se apaga e a pretensao de que ele DISTINGUE alguma coisa.
    conta = {}
    for s in secoes:
        for classe, caminhos in s["alcanca"].items():
            porta = porta_de(caminhos[0]["via"])
            conta[(classe, porta)] = conta.get((classe, porta), 0) + 1
    minimo = max(2, int(PORTA_COMUM * len(secoes)))
    portas = {k: v for k, v in conta.items() if v >= minimo}
    # A vizinhanca do corte vai IMPRESSA: um limiar escolhido em silencio e um
    # numero digitado a mao com outro nome. Vendo os que ficaram de fora por
    # pouco, quem le confere o corte em vez de acreditar nele.
    vizinhanca = sorted(((v, k[0], k[1]) for k, v in conta.items()),
                        reverse=True)[:8]

    for s in secoes:
        proprias = {}
        for classe, caminhos in s["alcanca"].items():
            sobra = [c for c in caminhos
                     if (classe, porta_de(c["via"])) not in portas]
            if sobra:
                proprias[classe] = sobra
        s["herda"] = sorted({classe for classe, caminhos in s["alcanca"].items()
                             if (classe, porta_de(caminhos[0]["via"])) in portas})
        s["proprias"] = proprias
        s["classe"] = classificar(
            {k for k, v in proprias.items() if v[0]["confianca"] >= CERTO},
            s["lacos_diretos"])
    return secoes, vizinhanca, minimo


ORDEM = ["codigo-do-dono", "rede-ou-espera", "escrita-duravel", "escrita",
         "leitura-com-varredura", "leitura-curta"]
# As classes que impedem uma secao de rodar sob leitor PARTILHADO.
ESCREVE = {"disco-escrita", "durabilidade", "usuario"}


def para_o_desenho(secoes):
    """Os numeros que a matriz de decisao cita -- daqui, e nao digitados.

    Cada um responde a UMA pergunta de desenho, e por isso nenhum e a
    contagem total: «quantas secoes existem» nao escolhe nada.
    """
    import statistics

    def certas(s, chave):
        return {k for k, v in s[chave].items() if v[0]["confianca"] >= CERTO}

    n = len(secoes)
    partilhaveis = [s for s in secoes if not (certas(s, "proprias") & ESCREVE)]
    sem_espelho = [s for s in partilhaveis if not (set(s["herda"]) & ESCREVE)]
    linhas = sorted(s["linhas"] for s in secoes)
    return [
        f"{len(partilhaveis)}/{n} secoes nao alcancam marcador de escrita por "
        "caminho proprio:",
        "     e o teto do que um RwLock poderia deixar rodar em paralelo",
        f"{len(sem_espelho)}/{n} idem, e tambem sem a porta comum do espelho "
        "(`recursos.espelho`):",
        "     com o espelho LIGADO, abrir tabela para ler pode sincronizar",
        f"{sum(1 for s in secoes if 'durabilidade' in certas(s, 'proprias'))}"
        f"/{n} alcancam `fsync` com a trava na mao:",
        "     e o que um RwLock NAO conserta -- o escritor continua exclusivo",
        f"{sum(1 for s in secoes if 'usuario' in certas(s, 'proprias'))}/{n} "
        "rodam codigo do DONO DO BANCO (gatilho BEFORE) com a trava na mao:",
        "     duracao sem teto, decidida por quem escreveu o gatilho",
        f"{sum(1 for s in secoes if s['solta_cedo'])}/{n} soltam a trava cedo, "
        "por `drop` explicito",
        f"{sum(1 for s in secoes if s['lacos_diretos'])}/{n} tem laco DIRETO "
        "dentro da secao critica",
        f"tamanho: menor {linhas[0]}, mediana {statistics.median(linhas):.0f}, "
        f"p90 {linhas[int(0.9 * len(linhas))]}, maior {linhas[-1]}, "
        f"soma {sum(linhas)} linhas",
    ]


def autoteste():
    """A prova real, nos dois sentidos: cada guarda FALHA com o defeito reposto.

    Medidor estatico e facil de acreditar e dificil de conferir -- ele nunca
    quebra, so passa a responder outra coisa. Estas seis provas repoem, uma a
    uma, os defeitos que este arquivo ja teve, e cada uma trava o conserto.
    """
    import tempfile
    falhas = []

    def confere(nome, valeu, detalhe=""):
        print(f"   {'ok  ' if valeu else 'FALHOU'} {nome}{detalhe and '  ' + detalhe}")
        if not valeu:
            falhas.append(nome)

    # 1. Comentario e texto nao sao codigo. O defeito: varrer o fonte cru -- e
    #    o proprio cabecalho deste arquivo, que escreve `sync_all`, viraria
    #    uma tomada de disco.
    fonte = 'fn a() {\n    // sync_all aqui\n    let s = "sync_all";\n}\n'
    confere("comentario e texto nao contam",
            not marcadores_em(sem_comentario_nem_texto(fonte)),
            f"cru acusa {sorted(marcadores_em(fonte))}")

    # 2. `'{'` e caractere, nao abertura de bloco. O defeito: contar chave no
    #    fonte cru -- o bloco fecharia no lugar errado e a secao critica sairia
    #    com o tamanho de outra coisa.
    fonte = "fn a() { let c = '{'; let d = '}'; }\n"
    limpo = sem_comentario_nem_texto(fonte)
    confere("literal de caractere nao vira chave",
            limpo.count("{") == 1 and limpo.count("}") == 1,
            f"{limpo.count('{')} aberturas, {limpo.count('}')} fechamentos")

    # 3. Fechadura local nao e funcao homonima de outro arquivo. O defeito
    #    real: o `juntar` do `juncao.rs` chamando a closure `montar` era
    #    classificado «atravessa a rede» pelo `Cliente::montar(TcpStream)`.
    indice = {"montar": ["{ TcpStream::connect(x); }"]}
    com_fechadura = "{ let montar = |a| a; montar(1); }"
    sem_fechadura = "{ montar(1); }"
    confere("fechadura local nao resolve para funcao de fora",
            not alcancaveis(com_fechadura, indice, 2)
            and "rede" in alcancaveis(sem_fechadura, indice, 2),
            "e a MESMA chamada, so muda a declaracao local")

    # 4. A profundidade e o que termina a recursao -- e ela tem de ALCANCAR o
    #    `fsync` do `op_inserir`, que esta a quatro saltos. O defeito: a guarda
    #    de ciclo por nome, que cortava em `sincronizar -> sincronizar`.
    indice = {"a": ["{ b(); }"], "b": ["{ c(); }"], "c": ["{ arq.sync_all(); }"]}
    confere("a cadeia de tres saltos e alcancada",
            "durabilidade" in alcancaveis("{ a(); }", indice, 4)
            and "durabilidade" not in alcancaveis("{ a(); }", indice, 1),
            "e nao com um salto so")

    # 5. Homonimo vira FRACAO, e nao um sim ou um nao. O defeito: unir todos
    #    (o `op_varrer` gravaria em disco) ou exigir unanimidade (o
    #    `op_inserir` nao faria `fsync`).
    meio = {"s": ["{ arq.sync_all(); }", "{ nada(); }"]}
    todo = {"s": ["{ arq.sync_all(); }", "{ outro.sync_all(); }"]}
    c_meio = alcancaveis("{ s(); }", meio, 2).get("durabilidade", [((), 0)])[0][1]
    c_todo = alcancaveis("{ s(); }", todo, 2).get("durabilidade", [((), 0)])[0][1]
    confere("homonimo vira confianca medida",
            abs(c_meio - 0.5) < 1e-9 and abs(c_todo - 1.0) < 1e-9,
            f"1 de 2 -> {c_meio:.2f}, 2 de 2 -> {c_todo:.2f}")

    # 6. Tomada em `#[cfg(test)]` nao e secao critica de producao. O defeito:
    #    procurar «mod testes» achava o primeiro dos DEZ deste arquivo e os
    #    outros nove entravam no mapa.
    with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
        f.write("impl S {\n    fn op(&self) {\n        let d = self.travar_dados()?;\n"
                "    }\n}\n#[cfg(test)]\nmod t1 { fn a() { s.travar_dados(); } }\n"
                "#[cfg(test)]\nmod t2 { fn b() { s.travar_dados(); } }\n")
        caminho = pathlib.Path(f.name)
    secoes, _, _ = mapear(caminho)
    caminho.unlink()
    confere("tomada dentro de #[cfg(test)] fica de fora",
            len(secoes) == 1 and secoes[0]["funcao"] == "op",
            f"achou {[s['funcao'] for s in secoes]}")

    print()
    if falhas:
        print(f"REPROVADO: {', '.join(falhas)}")
        return 1
    print("as seis guardas passaram")
    return 0


def principal():
    if "--autoteste" in sys.argv:
        print("=== as guardas do proprio medidor ===")
        return autoteste()
    secoes, vizinhanca, minimo = mapear()
    if "--json" in sys.argv:
        print(json.dumps({"total": len(secoes),
                          "corte_da_porta_comum": minimo,
                          "caminhos_mais_repetidos": [
                              {"secoes": n, "classe": c, "porta": v,
                               "herdado": n >= minimo}
                              for n, c, v in vizinhanca],
                          "secoes": secoes}, indent=2, ensure_ascii=False))
        return 0
    so = None
    if "--classe" in sys.argv:
        so = sys.argv[sys.argv.index("--classe") + 1]

    print("=== o mapa da trava global de dados ===")
    print(f"    fonte: {ALVO.relative_to(RAIZ)}")
    print(f"    {len(secoes)} secoes criticas fora da definicao e fora dos testes")
    print(f"    profundidade: {SALTOS} saltos, resolucao por nome\n")
    print(f"-- as portas mais repetidas, e o corte da PORTA COMUM ({minimo} secoes)")
    print("   herdado = e a melhor prova em tanta secao que nao distingue nenhuma;")
    print("   continua sendo fato, sai da classificacao. Ver PORTA_COMUM.")
    for n, classe, porta in vizinhanca:
        selo = "HERDADO" if n >= minimo else "       "
        print(f"   {selo} {n:>3}/{len(secoes)}  {classe:<15} "
              f"pela porta {porta or 'aqui mesmo'}")
    print()

    for classe in ORDEM:
        desta = [s for s in secoes if s["classe"] == classe]
        if not desta or (so and classe != so):
            continue
        print(f"-- {classe}: {len(desta)}")
        for s in sorted(desta, key=lambda x: -x["linhas"]):
            extra = " (solta cedo)" if s["solta_cedo"] else ""
            print(f"   {s['linha']:>6}  {s['funcao']:<38} "
                  f"{s['linhas']:>5} linhas{extra}")
            for k, v in sorted(s["proprias"].items()):
                c = v[0]
                por = " -> ".join(c["via"]) if c["via"] else "aqui mesmo"
                selo = "  " if c["confianca"] >= CERTO else f" [{c['confianca']:.2f}]"
                print(f"           {k:<16}{selo} via {por}")
            if s["herda"]:
                print(f"           herda das portas comuns: {', '.join(s['herda'])}")
        print()

    print("-- o que isto significa para o desenho substituto")
    for linha in para_o_desenho(secoes):
        print(f"   {linha}")
    print()
    print("-- resumo")
    for classe in ORDEM:
        n = sum(1 for s in secoes if s["classe"] == classe)
        linhas = sum(s["linhas"] for s in secoes if s["classe"] == classe)
        print(f"   {classe:<24} {n:>3} secoes   {linhas:>6} linhas de codigo sob a trava")
    return 0


if __name__ == "__main__":
    # O mapa e longo e se le com `| head`. Sem isto, fechar o cano faz o Python
    # cuspir um `BrokenPipeError` no fim de uma saida perfeita -- e um medidor
    # que termina com pilha de erro parece um medidor que falhou.
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    raise SystemExit(principal())
