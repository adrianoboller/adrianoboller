#!/usr/bin/env python3
"""O MAPA das threads: onde cada uma nasce, e qual teto a segura.

    python3 bancada/concorrencia/mapa-das-threads.py             # o mapa legivel
    python3 bancada/concorrencia/mapa-das-threads.py --json      # para outro gerador
    python3 bancada/concorrencia/mapa-das-threads.py --catraca   # spawn-sem-teto = 0
    python3 bancada/concorrencia/mapa-das-threads.py --numeros   # para o inventario
    python3 bancada/concorrencia/mapa-das-threads.py --autoteste # as guardas do medidor

Por que este medidor existe
---------------------------
Pedido 248: «multi threads devem ter um controle altamente validado com
semaforos adequados». O controle e o `Semaforo` do `phxsql-core`; a VALIDACAO
e este arquivo. Ele le o fonte, acha todo lugar onde uma thread nasce fora dos
testes e exige que cada um esteja no CATALOGO abaixo com o seu teto -- ou com a
dispensa e o motivo. Thread nova que alguem subir sem passar por aqui reprova a
bateria antes de qualquer servidor subir.

O que ele aprendeu antes de existir: o quadro da rodada de 16/09 dizia que o
fecho da janela tinha «K fios, sem teto». Tinha teto (`FIOS_DO_FECHO = 16`),
28 linhas acima do `thread::scope` -- o `grep` achou o spawn e nao o teto,
porque o teto de uma thread quase nunca esta na linha em que ela nasce. Por
isso o catalogo e escrito A MAO, com o teto e a linha onde ele mora, e o
medidor so confere que todo spawn tem entrada: o que uma maquina consegue
achar e o spawn; o que segura o spawn, alguem tem de ler.

O que ele conta, e o que NAO conta
----------------------------------
CONTA: `thread::spawn(`, `thread::Builder::new(`, `thread::scope(` e
`.subir(` (o registro da telemetria, que e o unico caminho de producao ate o
`Builder`) em `crates/*/src`, fora do modulo de testes de cada arquivo
(`#[cfg(test)] mod ...` em diante) e fora dos arquivos que so existem em teste
(`#[cfg(test)] mod x;` no `lib.rs`). Comentario e literal de texto viram
espaco antes da varredura, com a mesma funcao do `mapa-da-trava.py`.

NAO CONTA: threads que nascem em `examples/`, `tests/` e `bancada/` -- sao
medidores, e um medidor sem teto e problema de quem mede, nao do servidor.

A catraca
---------
`spawn-sem-teto` = sitios sem entrada no catalogo, ou com entrada sem teto.
Teto e ZERO e nao e um numero como os outros: um so ja e uma enxurrada
possivel. Ela nunca sobe. Entrada do catalogo que nao casa com sitio nenhum
tambem reprova -- e a entrada que envelheceu, e catalogo velho e pior que
catalogo nenhum porque parece completo.
"""
import importlib.util
import json
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
CRATES = RAIZ / "crates"

# A mesma faca do mapa-da-trava: comentario e literal viram espaco, posicao
# preservada. Importado do arquivo (o nome tem hifen) para nao haver duas
# copias que divergem na primeira correcao feita numa so.
_esp = importlib.util.spec_from_file_location(
    "mapa_da_trava", Path(__file__).resolve().parent / "mapa-da-trava.py")
_mapa_da_trava = importlib.util.module_from_spec(_esp)
_esp.loader.exec_module(_mapa_da_trava)
sem_comentario_nem_texto = _mapa_da_trava.sem_comentario_nem_texto

# Como uma thread nasce. A chave e o rotulo do mapa; o valor, a agulha no
# texto LIMPO (sem comentario nem literal).
NASCIMENTOS = {
    "thread::spawn": re.compile(r"\bthread::spawn\s*\("),
    "Builder::new": re.compile(r"\bthread::Builder::new\s*\(|\bBuilder::new\s*\(\s*\)\s*\.name"),
    "thread::scope": re.compile(r"\bthread::scope\s*\("),
    "telemetria.subir": re.compile(r"\.subir\s*\("),
}
# Daqui em diante e teste: `#[cfg(test)]` seguido de `mod x {`.
MODULO_DE_TESTE = re.compile(r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{")
# `#[cfg(test)] mod x;` no lib.rs: o arquivo `x.rs` inteiro so existe em teste.
ARQUIVO_DE_TESTE = re.compile(r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;")
# Quantas linhas em volta do sitio a agulha do catalogo pode estar: o nome da
# thread vem 1-2 linhas depois do `subir(`, e o `let nome = format!(...)`
# da replica vem 2 linhas antes.
JANELA_ANTES, JANELA_DEPOIS = 6, 4

# ----------------------------------------------------------------- o catalogo
#
# Cada sitio de producao, com o teto que o segura e ONDE o teto mora. `teto`
# vazio e sitio sem teto, e reprova a catraca. `agulha` e o texto que tem de
# aparecer na janela em volta do spawn, no fonte ORIGINAL (com literais).
CATALOGO = [
    # ------------------------------------------------ o mecanismo, um so
    {
        "arquivo": "crates/phxsql-server/src/telemetria.rs",
        "agulha": "std::thread::Builder::new().name(nome_do_so)",
        "nome": "telemetria::subir",
        "teto": "e o unico `spawn` de producao do servidor: toda thread passa "
                "por aqui, e o teto de cada uma e o da chamada a `subir` que a "
                "pediu (as entradas abaixo). A ficha morre no `Drop` "
                "(`FichaViva`), inclusive em panico.",
    },
    # ------------------------------------------------ atendimento, com semaforo
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": 'format!("dados-{}", endereco.port())',
        "nome": "dados-<porta>",
        "teto": "`recursos.conexoes_max` (64) pelo `Semaforo permissoes_de_dados`: "
                "`tentar()` no `accept`, recusa IMEDIATA e anotada no "
                "`acessos.log`; a `Permissao` viaja para dentro da thread e "
                "morre com ela, inclusive em panico (pedido 248).",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": 'format!("{familia}-{}", par.port())',
        "nome": "web-/rest-/swagger-<porta>",
        "teto": "`recursos.conexoes_web_max` (64; 0 = sem teto) pelo `Semaforo "
                "permissoes_http`, UM para as tres portas HTTP: `vaga_http()` "
                "espera ate `recursos.fila_web_ms` (2.000) e, estourada, 503 "
                "com `Retry-After` do proprio aceitador, sem subir thread "
                "(pedido 248).",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": 'format!("ouvinte-{familia}")',
        "nome": "ouvinte-web / ouvinte-rest / ouvinte-swagger",
        "teto": "uma por porta HTTP ligada (ate 3): so aceita, nunca atende. "
                "Ate o pedido 248 a interface tinha uma copia propria deste "
                "laco; hoje e' o mesmo `aceitar_http`.",
    },
    # ------------------------------------------------ paralelismo com teto medido
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": "std::thread::scope(|escopo| {",
        "nome": "fecho da janela (um fio por tabela suja)",
        "teto": "`FIOS_DO_FECHO = 16` por pedaco (`lista.chunks(FIOS_DO_FECHO)`, "
                "28 linhas acima do `scope`); pedacos em serie. MEDIDO em "
                "16/09/2026 com `--example o-comboio-em-paralelo --tetos "
                "4,8,16,0` em K=16, tres corridas limpas: teto 4 custa +34% a "
                "+68%, teto 8 custa +19%/+25%/+0,1%, e «16» contra «sem» -- o "
                "mesmo codigo -- divergiu ate 27,6%, que e' o ruido. Nenhum "
                "teto menor passa nos 5%: fica 16. Ver docs/CONCORRENCIA.md §17.",
    },
    {
        "arquivo": "crates/phxsql-core/src/paralelo.rs",
        "agulha": "std::thread::scope(|escopo| {",
        "nome": "paralelo::mapear_faixa (varredura em memoria)",
        "teto": "`paralelo::nucleos()` = min(nucleos da maquina, "
                "`recursos.threads`) x `recursos.cpu_percentual`, e nunca mais "
                "fios que `n / MINIMO_PARA_DIVIDIR` (50.000). Nao e' semaforo: "
                "as threads morrem no fim do `scope`, entao o teto e' o numero "
                "delas, e ele ja era lido do config antes do pedido 248.",
    },
    # ------------------------------------------------ servicos, um de cada
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"relogio-gravacao"',
        "nome": "relogio-gravacao",
        "teto": "1 (sobe uma vez no arranque)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"amostrador"',
        "nome": "amostrador",
        "teto": "1 (sobe uma vez no arranque)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"pulso-supervisor"',
        "nome": "pulso-supervisor",
        "teto": "1 (sobe uma vez, so com `cluster` no config)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"arbitro-cluster"',
        "nome": "arbitro-cluster",
        "teto": "1 (sobe uma vez, so com `cluster` no config)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"replica-cluster"',
        "nome": "replica-cluster",
        "teto": "1 (sobe uma vez, so com `cluster` no config)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": 'format!("pulso-{}", no.id)',
        "nome": "pulso-<no>",
        "teto": "uma por no' declarado em `cluster.nos` (lista fixa do config)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": 'format!("replica-{}", origem.nome)',
        "nome": "replica-<origem>",
        "teto": "uma por origem em `replicacao.origens` (lista fixa do config)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"backup-agendado"',
        "nome": "backup-agendado",
        "teto": "1 (sobe uma vez, so com `backup.agendado`)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"relogio-jobs"',
        "nome": "relogio-jobs",
        "teto": "1 (sobe uma vez; `relogio_de_jobs` e' um `AtomicBool` que "
                "impede a segunda)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"vigia-jobs"',
        "nome": "vigia-jobs",
        "teto": "1 (sobe uma vez no arranque)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"vigia-disco"',
        "nome": "vigia-disco",
        "teto": "1 (sobe uma vez no arranque)",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"sonda-disco"',
        "nome": "sonda-disco",
        "teto": "1 (sobe uma vez no arranque, com `alertas.disco.ligado` ou "
                "`alertas.email.ligado`; pedido 249). E a sonda E o carteiro: "
                "quem registra um evento so entrega a fila (`SaudeDoDisco::"
                "entregar`, sem rede, porque pode estar com a trava de dados na "
                "mao), e esta thread acorda por `Condvar` e fala com o rele. O "
                "teto mora no `if` de `ligar_sonda_de_disco`, que roda uma vez "
                "no `servir`.",
    },
    # ------------------------------------------------ uma por EVENTO, com silencio
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"aviso-seguranca"',
        "nome": "aviso-seguranca",
        "teto": "uma por IP bloqueado a cada `alertas.repetir_horas` (6 h), "
                "pelo `pode_avisar` que roda ANTES do `subir`; vive o tempo de "
                "UM envio ao rele. O teto e' o silencio, nao um numero -- e "
                "quem o apertar mede o rele antes.",
    },
    {
        "arquivo": "crates/phxsql-server/src/servidor.rs",
        "agulha": '"aviso-job"',
        "nome": "aviso-job",
        "teto": "uma por job que falhou a cada `alertas.repetir_horas` (6 h), "
                "pelo `pode_avisar` que roda ANTES do `subir`; vive o tempo de "
                "UM envio ao rele.",
    },
]

# --------------------------------------------------------------- a varredura


def arquivos_so_de_teste(raiz_do_crate):
    """Os `x.rs` que o `lib.rs`/`main.rs` declara com `#[cfg(test)] mod x;`."""
    nomes = set()
    for entrada in ("lib.rs", "main.rs"):
        f = raiz_do_crate / "src" / entrada
        if f.exists():
            for m in ARQUIVO_DE_TESTE.finditer(f.read_text(encoding="utf-8")):
                nomes.add(m.group(1))
    return nomes


def fontes():
    """Os `.rs` de producao de todos os crates."""
    saida = []
    for crate in sorted(CRATES.iterdir()):
        src = crate / "src"
        if not src.is_dir():
            continue
        de_teste = arquivos_so_de_teste(crate)
        for rs in sorted(src.rglob("*.rs")):
            if rs.stem in de_teste:
                continue
            saida.append(rs)
    return saida


def rotulo_de(arquivo):
    """Relativo a raiz do repositorio; fora dela (o autoteste usa arquivos
    temporarios), so o nome -- e o catalogo do autoteste casa pelo nome."""
    try:
        return str(arquivo.relative_to(RAIZ))
    except ValueError:
        return arquivo.name


def sitios_de(arquivo):
    """Os nascimentos de thread de UM arquivo, fora do modulo de testes."""
    original = arquivo.read_text(encoding="utf-8")
    limpo = sem_comentario_nem_texto(original)
    corte = MODULO_DE_TESTE.search(limpo)
    fim = corte.start() if corte else len(limpo)
    linhas_originais = original.split("\n")
    achados = []
    for tipo, agulha in NASCIMENTOS.items():
        for m in agulha.finditer(limpo, 0, fim):
            linha = limpo.count("\n", 0, m.start()) + 1
            # A definicao do proprio `subir` nao e um sitio: e o mecanismo.
            trecho = linhas_originais[linha - 1]
            if tipo == "telemetria.subir" and re.search(r"\bfn\s+subir\b", trecho):
                continue
            ini = max(0, linha - 1 - JANELA_ANTES)
            janela = "\n".join(linhas_originais[ini:linha - 1 + JANELA_DEPOIS])
            achados.append({
                "arquivo": rotulo_de(arquivo),
                "linha": linha,
                "tipo": tipo,
                "janela": janela,
            })
    return achados


def mapear(arquivos=None, catalogo=None):
    """Todos os sitios, cada um casado (ou nao) com a entrada do catalogo."""
    catalogo = CATALOGO if catalogo is None else catalogo
    sitios = []
    for arq in (arquivos or fontes()):
        sitios.extend(sitios_de(arq))
    usadas = set()
    for s in sitios:
        s["entrada"] = None
        for i, e in enumerate(catalogo):
            if e["arquivo"] == s["arquivo"] and e["agulha"] in s["janela"]:
                s["entrada"] = i
                usadas.add(i)
                break
    envelhecidas = [e for i, e in enumerate(catalogo) if i not in usadas]
    return sitios, envelhecidas


def medir_para_a_catraca(sitios, envelhecidas, catalogo=None):
    """Os numeros da catraca, contados AQUI e nao lidos do texto impresso."""
    catalogo = CATALOGO if catalogo is None else catalogo
    sem_teto = 0
    for s in sitios:
        if s["entrada"] is None or not catalogo[s["entrada"]].get("teto"):
            sem_teto += 1
    return {
        "spawn-sem-teto": sem_teto,
        "catalogo-envelhecido": len(envelhecidas),
        "sitios": len(sitios),
    }


# ----------------------------------------------------------------- a catraca
#
# CATRACA SO DESCE, e esta ja nasce no chao: zero sitios sem teto, zero
# entradas envelhecidas. Nao ha «baixe o teto» aqui porque nao ha para onde.
# A quarta coluna e o rotulo curto do inventario (`docs/qa/medir.py`, pelo
# `--numeros`). Ele nao e regua nem teto: e o «o que esta catraca mede» que a
# tabela gerada imprime ao lado do numero, para que ela nao precise de uma
# segunda lista de descricoes que envelheceria sozinha.
CATRACAS = [
    (
        "spawn-sem-teto",
        0,
        "sitios onde uma thread nasce sem teto declarado no catalogo deste "
        "arquivo. Um so ja e uma enxurrada possivel -- foi assim que a porta "
        "web viveu sem teto ate o pedido 248, com o proprio comentario "
        "confessando.",
        "sitios de nascimento de thread sem teto no catalogo",
    ),
    (
        "catalogo-envelhecido",
        0,
        "entradas do catalogo que nao casam com sitio nenhum: alguem mexeu no "
        "fonte e o catalogo ficou descrevendo uma thread que nao nasce mais "
        "ali. Catalogo velho e pior que catalogo nenhum, porque parece completo.",
        "entradas do catalogo de threads que nao casam com sitio nenhum",
    ),
]


def catraca():
    sitios, envelhecidas = mapear()
    medido = medir_para_a_catraca(sitios, envelhecidas)
    print("=== a catraca do mapa das threads ===")
    print(f"    {medido['sitios']} sitios de nascimento fora dos testes, em crates/*/src\n")
    reprovou = False
    for nome, teto, porque, _mede in CATRACAS:
        agora = medido[nome]
        estado = "ok" if agora <= teto else "SUBIU"
        reprovou |= agora > teto
        print(f"   {estado:<8} {nome:<22} {agora:>3} (teto {teto})")
        if agora > teto:
            print(f"        {porque}")
    if reprovou:
        print()
        for s in sitios:
            if s["entrada"] is None or not CATALOGO[s["entrada"]].get("teto"):
                print(f"   SEM TETO  {s['arquivo']}:{s['linha']}  {s['tipo']}")
        for e in envelhecidas:
            print(f"   ENVELHECIDA  {e['arquivo']}  agulha {e['agulha']!r}")
        print("\nREPROVADO: ponha o sitio no CATALOGO com o teto e onde ele mora, "
              "ou com a dispensa e o motivo.")
        return 1
    print("\nAs duas catracas seguram.")
    return 0


# ----------------------------------------------------------------- autoteste
#
# Medidor estatico nunca quebra -- passa a responder outra coisa. Cada guarda
# aqui repoe um defeito que este medidor PODE ter, e as tres primeiras sao
# exatamente as que o mapa-da-trava ja teve.


def autoteste():
    import tempfile
    falhas = []

    def confere(nome, cond):
        print(f"   {'ok' if cond else 'FALHOU'}  {nome}")
        if not cond:
            falhas.append(nome)

    with tempfile.TemporaryDirectory() as d:
        d = Path(d)

        # 1. Um spawn em producao sem entrada e' contado como SEM TETO.
        a = d / "a.rs"
        a.write_text("fn x() {\n    std::thread::spawn(|| {});\n}\n")
        sitios, _ = mapear([a], catalogo=[])
        confere("spawn em producao sem entrada conta como sem teto",
                len(sitios) == 1 and medir_para_a_catraca(sitios, [], [])["spawn-sem-teto"] == 1)

        # 2. O mesmo spawn DENTRO do modulo de testes nao conta.
        b = d / "b.rs"
        b.write_text("fn x() {}\n#[cfg(test)]\nmod testes {\n    fn y() { std::thread::spawn(|| {}); }\n}\n")
        sitios, _ = mapear([b], catalogo=[])
        confere("spawn dentro de `#[cfg(test)] mod` nao conta", len(sitios) == 0)

        # 3. Comentario e literal nao sao codigo.
        c = d / "c.rs"
        c.write_text('fn x() {\n    // std::thread::spawn(|| {});\n    let s = "thread::scope(";\n}\n')
        sitios, _ = mapear([c], catalogo=[])
        confere("spawn em comentario ou literal nao conta", len(sitios) == 0)

        # 4. Uma entrada com teto casa pela agulha na JANELA em volta, e ai o
        #    sitio deixa de ser sem teto.
        e = d / "e.rs"
        e.write_text('fn x() {\n    let nome = "vigia-x";\n    t.subir(\n        nome,\n        "faz",\n    );\n}\n')
        cat = [{"arquivo": "e.rs", "agulha": '"vigia-x"', "nome": "vigia-x", "teto": "1"}]
        sitios, envelhecidas = mapear([e], catalogo=cat)
        m = medir_para_a_catraca(sitios, envelhecidas, cat)
        confere("entrada com teto casa pela agulha na janela e zera o sem-teto",
                len(sitios) == 1 and m["spawn-sem-teto"] == 0 and m["catalogo-envelhecido"] == 0)

        # 5. Entrada que nao casa com nada e' ENVELHECIDA, e reprova.
        cat2 = [{"arquivo": "e.rs", "agulha": '"vigia-que-nao-existe"', "nome": "?", "teto": "1"}]
        sitios, envelhecidas = mapear([e], catalogo=cat2)
        confere("entrada sem sitio e' envelhecida e reprova",
                medir_para_a_catraca(sitios, envelhecidas, cat2)["catalogo-envelhecido"] == 1)

        # 6. A definicao do proprio `fn subir(` nao e' um sitio.
        f = d / "f.rs"
        f.write_text("impl T {\n    pub fn subir(&self) {}\n}\nfn g(t: &T) { t.subir(); }\n")
        sitios, _ = mapear([f], catalogo=[])
        confere("`fn subir(` e' o mecanismo, nao um sitio; a chamada e'",
                len(sitios) == 1 and sitios[0]["linha"] == 4)

    # 7. E contra o fonte de verdade: a catraca do repositorio esta em zero.
    sitios, envelhecidas = mapear()
    m = medir_para_a_catraca(sitios, envelhecidas)
    confere(f"no fonte de verdade: {m['sitios']} sitios, {m['spawn-sem-teto']} sem teto, "
            f"{m['catalogo-envelhecido']} envelhecidas",
            m["spawn-sem-teto"] == 0 and m["catalogo-envelhecido"] == 0)

    if falhas:
        print(f"REPROVADO: {', '.join(falhas)}")
        return 1
    print("as sete guardas passaram")
    return 0


# ----------------------------------------------------------------- o mapa


EU = str(Path(__file__).resolve().relative_to(RAIZ))


def numeros():
    """Saida de maquina para o `docs/qa/medir.py` -- o inventario das catracas.

    Ele NAO le a prosa do `--catraca`: `grep` em relatorio e resolver numero
    por comparacao de FRASE, e no dia em que alguem melhorar a redacao o
    inventario publica o numero de ontem sem dizer nada. A chave e estavel;
    o rotulo e livre.

    Este modo nasceu em 16/09/2026, e o buraco que ele fecha estava medido no
    `docs/CATRACAS.md`: o inventario gerado varria `crates/*/examples/*.rs` e
    `crates/*/src/**`, e as catracas que moram em `bancada/`, em Python, nao
    apareciam nele -- a tabela que existe para dizer quantas catracas ha
    contava menos do que existe.

    Sai SEMPRE com codigo 0, mesmo com catraca vermelha: este modo RELATA, e
    quem reprova e o `--catraca`. Um codigo de saida != 0 aqui faria o
    inventario dizer «nao consegui medir» justamente na catraca que esta
    vermelha -- e ela sumiria da tabela em vez de aparecer vermelha."""
    sitios, envelhecidas = mapear()
    medido = medir_para_a_catraca(sitios, envelhecidas)
    for nome, teto, _porque, mede in CATRACAS:
        print(f"catraca:nome={nome};onde={EU};valor={teto};"
              f"medido={medido[nome]};tipo=teto;mede={mede}")
    return 0


def principal():
    if "--numeros" in sys.argv:
        return numeros()
    if "--catraca" in sys.argv:
        return catraca()
    if "--autoteste" in sys.argv:
        print("=== as guardas do proprio medidor ===")
        return autoteste()
    sitios, envelhecidas = mapear()
    if "--json" in sys.argv:
        for s in sitios:
            e = CATALOGO[s["entrada"]] if s["entrada"] is not None else None
            s["nome"] = e["nome"] if e else None
            s["teto"] = e["teto"] if e else None
            del s["janela"]
        print(json.dumps({
            "total": len(sitios),
            "catraca": medir_para_a_catraca(sitios, envelhecidas),
            "sitios": sitios,
            "envelhecidas": envelhecidas,
        }, indent=2, ensure_ascii=False))
        return 0

    print("=== o mapa das threads ===")
    print(f"    fonte: crates/*/src, fora dos testes")
    print(f"    {len(sitios)} sitios de nascimento; catalogo com {len(CATALOGO)} entradas\n")
    print(f"   {'onde':<44} {'como':<17} quem e o teto")
    print(f"   {'-' * 44} {'-' * 17} {'-' * 40}")
    for s in sorted(sitios, key=lambda x: (x["arquivo"], x["linha"])):
        onde = f"{s['arquivo'].split('/')[-1]}:{s['linha']}"
        if s["entrada"] is None:
            print(f"   {onde:<44} {s['tipo']:<17} SEM ENTRADA NO CATALOGO")
            continue
        e = CATALOGO[s["entrada"]]
        teto = e["teto"] or "SEM TETO"
        print(f"   {onde:<44} {s['tipo']:<17} {e['nome']}")
        # o teto, em linhas de ate 70 colunas
        palavras, linha = teto.split(), ""
        for p in palavras:
            if len(linha) + len(p) + 1 > 70:
                print(f"   {'':<62} {linha}")
                linha = p
            else:
                linha = f"{linha} {p}".strip()
        if linha:
            print(f"   {'':<62} {linha}")
    if envelhecidas:
        print("\n-- entradas do catalogo que nao casam com sitio nenhum (ENVELHECIDAS)")
        for e in envelhecidas:
            print(f"   {e['arquivo']}  {e['agulha']!r}")
    m = medir_para_a_catraca(sitios, envelhecidas)
    print(f"\n-- resumo: {m['sitios']} sitios, {m['spawn-sem-teto']} sem teto, "
          f"{m['catalogo-envelhecido']} entradas envelhecidas")
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
