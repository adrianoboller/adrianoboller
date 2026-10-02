#!/usr/bin/env python3
"""Levantamento das variaveis PHXCLAW_* do fonte contra o catalogo do config.json.

Reprova (codigo 1) em tres casos, cada um com arquivo e linha:

1. variavel PHXCLAW_* no fonte .rs que NAO esta no catalogo -- configuracao que a tela nao
   mostra e que o config.json nao alcanca;
2. variavel do catalogo que ninguem mais menciona no fonte -- configuracao que nao e lida
   mente, e a tela ofereceria um campo sem efeito;
3. a CATRACA das leituras soltas: o numero de pontos do fonte (fora do modulo de
   configuracao) que nomeiam uma variavel PHXCLAW_* para LER, comparado ao TETO. Subiu,
   reprova: leitura nova entra por `config::valor`. Desceu, reprova tambem, pedindo para
   baixar o TETO no mesmo passo -- catraca frouxa nao segura nada.

O catalogo e lido do artefato gerado (apps/phxclaw-ui/assets/config-catalogo.json), que o
teste `artefatos_gerados_estao_em_dia` mantem igual ao catalogo em codigo.

Nomes montados em tempo de execucao tambem entram: os canais (`PHXCLAW_<PREFIXO>_<CHAVE>`,
lidos do `canais/ligar.rs`, braco a braco) e as forjas (`PHXCLAW_<FORJA>_TOKEN`). Um
`format!("PHXCLAW_{...}")` novo que este script nao sabe expandir e reprovado tambem:
silencio ai seria um buraco no levantamento.

Uso: python3 tools/config_catalogo.py [--raiz DIR] [--listar]
"""
import argparse
import json
import re
import sys
from pathlib import Path

# Leituras soltas medidas em 01/10/2026, no inicio da fase 1 da centralizacao: 135 pontos.
# So desce: cada leva da fase 2 que migra leitores para `config::valor` baixa este numero
# no mesmo passo, ate 0. 134: o `pasta()` da CLI passou a usar `config::pasta_padrao`
# (SP000028, conserto da pasta fixada). 127: as variaveis dos `Servico` (chaves.rs) passaram
# a sair do catalogo (`por_chave`) em vez de uma segunda lista, e os leitores de
# PHXCLAW_API_TOKEN, PHXCLAW_PONTE_TOKEN, PHXCLAW_ENROLLMENT_TOKEN e PHXCLAW_IMAGEM_CHAVE
# leem pelo `Servico` (SP000013, frente C). 5: SP000020 (fase 2) migrou os 122 leitores
# restantes -- o agente le por `config::texto_de`/`inteiro_de`/`lista_de`/`caminho_de`/
# `segredo_do_ambiente`, o nome citado ao operador sai de `config::variavel(chave)`, e os
# crates sem o agente (pontes, navegador, busca, SDK, missao, no de dispositivo, desktop)
# leem pelo leitor de processo do config-runtime (`carga::*_do_processo`). Os 5 que ficam,
# cada um com o motivo:
#   - canais/ligar.rs:66 `format!("PHXCLAW_{}_{k}")`: o nome e MONTADO por canal e lido por
#     um leitor injetado (a CLI passa `config::por_variavel`, que resolve pelo catalogo);
#     o braco dividido (whatsapp|messenger) faz a chave nao ser 1:1 com a variavel, entao
#     trocar a chave do leitor pelo nome do catalogo e a fase 3, nao um `replace`;
#   - lsp.rs:199 e pwa.rs:21: arquivos da frente W2 (VS Code, em curso em 02/10/2026) -- nao
#     se toca em arvore alheia; migram quando a frente fechar (`config::booleano_de("agente.lsp")`
#     e `config::caminho_de("ui.dir")`, respectivamente);
#   - apps/phxclaw-desktop/src-tauri/src/terminal.rs:65 e :91: idem (frente W1/W2, Tauri);
#     a leitura e `carga::caminho_do_processo("agente.projeto")` e `("desktop.hx")`.
TETO = 5

NOME = re.compile(r"\bPHXCLAW_[A-Z0-9_]*[A-Z0-9]\b")
LITERAL = re.compile(r'"(PHXCLAW_[A-Z0-9_]*[A-Z0-9])"')
MONTADO = re.compile(r'format!\(\s*"PHXCLAW_\{')
# Exportar ao processo filho nao e ler: `("PHXCLAW_X".into(), valor)`, `.env("PHXCLAW_X"`.
# O script Python que o agente escreve para o filho le `os.environ["PHXCLAW_X"]` -- e o
# filho lendo o que o agente exportou.
EXPORTA = re.compile(
    r'\(\s*"PHXCLAW_[A-Z0-9_]+"\s*\.into\(\)\s*,|\.env(_remove)?\(\s*"PHXCLAW_|os\.environ\['
)

# Casam o padrao e nao sao variavel de ambiente.
NAO_SAO_VARIAVEIS = {
    "PHXCLAW_FIM": "marca de heredoc do shell (crates/phxclaw-agent/src/python.rs)",
}

# O modulo de configuracao: e o unico lugar onde ler o ambiente e o certo.
MODULO_DE_CONFIG = (
    "crates/phxclaw-config-runtime/",
    "crates/phxclaw-agent/src/config.rs",
    "apps/phxclaw/src/config.rs",
)


def fontes(raiz):
    for p in sorted(raiz.rglob("*.rs")):
        rel = p.relative_to(raiz).as_posix()
        if rel.startswith(("target/", "third_party/")) or "/target/" in rel:
            continue
        yield rel, p.read_text(encoding="utf-8", errors="replace")


def eh_producao(rel):
    partes = rel.split("/")
    return "src" in partes and "tests" not in partes and "examples" not in partes and partes[-1] != "tests.rs"


def corpo_de_producao(texto):
    """As linhas antes do primeiro `#[cfg(test)]` (o modulo de testes fica no fim)."""
    linhas = texto.splitlines()
    for i, l in enumerate(linhas):
        if l.strip().startswith("#[cfg(test)]"):
            return linhas[:i]
    return linhas


def bracos_dos_canais(texto):
    """Cada braco `"a" | "b" => { ... }` do `ligar`: (canais, corpo)."""
    out = []
    for m in re.finditer(r'^\s*((?:"[a-z]+"\s*\|\s*)*"[a-z]+")\s*=>\s*\{', texto, re.M):
        canais = re.findall(r'"([a-z]+)"', m.group(1))
        i = m.end()
        nivel = 1
        while nivel and i < len(texto):
            nivel += {"{": 1, "}": -1}.get(texto[i], 0)
            i += 1
        out.append((canais, texto[m.end():i]))
    return out


def dinamicos(raiz, erros):
    """Os nomes montados: canais e forjas. Devolve {nome: onde}."""
    nomes = {}
    ligar = raiz / "crates/phxclaw-agent/src/canais/ligar.rs"
    t = ligar.read_text(encoding="utf-8")
    lista = re.search(r"pub const CANAIS: &\[&str\] = &\[(.*?)\];", t, re.S)
    canais = re.findall(r'"([a-z]+)"', lista.group(1)) if lista else []
    prefixo = {c: c.upper() for c in canais}
    for c, p in re.findall(r'"([a-z]+)" => "([A-Z_]+)"\.to_string\(\)', t):
        prefixo[c] = p
    for nome_canais, corpo in bracos_dos_canais(t):
        if not set(nome_canais) <= set(canais) or nome_canais == ["telegram"]:
            continue
        ks = set(re.findall(r'ctx\s*\.\s*(?:cfg|exigir|segredo|segredo_opcional|segredo_de)\(\s*"([A-Z_]+)"', corpo))
        if re.search(r"ctx\s*\.\s*base\(", corpo):
            ks.add("BASE")
        if re.search(r"ctx\s*\.\s*tls\(", corpo):
            ks |= {"TLS", "CA"}
        for k in ks:
            # Braco dividido (whatsapp|messenger): a chave vale para pelo menos um deles.
            nomes.setdefault(("|".join(f"PHXCLAW_{prefixo[c]}_{k}" for c in nome_canais)), "canais/ligar.rs")
    for c in canais:
        if c != "telegram":
            nomes[f"PHXCLAW_{prefixo[c]}_PERMITIDOS"] = "canais/ligar.rs"
    forja = (raiz / "crates/phxclaw-agent/src/forja.rs").read_text(encoding="utf-8")
    for f in re.findall(r'Forja::\w+ => "([a-z]+)",', forja):
        nomes[f"PHXCLAW_{f.upper()}_TOKEN"] = "forja.rs"
    return nomes


CONHECIDOS = {
    "crates/phxclaw-agent/src/canais/ligar.rs",
    "crates/phxclaw-agent/src/forja.rs",
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--raiz", default=str(Path(__file__).resolve().parent.parent))
    ap.add_argument("--listar", action="store_true", help="lista as leituras soltas")
    a = ap.parse_args()
    raiz = Path(a.raiz)
    cat = json.loads((raiz / "apps/phxclaw-ui/assets/config-catalogo.json").read_text(encoding="utf-8"))
    no_catalogo = {c["variavel"]: c for c in cat["chaves"]}
    erros = []
    vistos = {}
    soltas = []
    for rel, texto in fontes(raiz):
        for n, linha in enumerate(texto.splitlines(), 1):
            for nome in NOME.findall(linha):
                vistos.setdefault(nome, f"{rel}:{n}")
            if MONTADO.search(linha) and rel not in CONHECIDOS and not rel.startswith(MODULO_DE_CONFIG):
                erros.append(f"{rel}:{n}: nome PHXCLAW_ montado em tempo de execucao que o levantamento nao sabe expandir")
        if not eh_producao(rel) or rel.startswith(MODULO_DE_CONFIG):
            continue
        for n, linha in enumerate(corpo_de_producao(texto), 1):
            if EXPORTA.search(linha):
                continue
            if LITERAL.search(linha) or MONTADO.search(linha):
                soltas.append(f"{rel}:{n}: {linha.strip()}")
    for alternativas, onde in dinamicos(raiz, erros).items():
        opcoes = alternativas.split("|")
        achado = [o for o in opcoes if o in no_catalogo]
        if not achado:
            erros.append(f"{onde}: {' ou '.join(opcoes)} montada no fonte e fora do catalogo")
        for o in achado:
            vistos.setdefault(o, onde)
    for nome, onde in sorted(vistos.items()):
        if nome not in no_catalogo and nome not in NAO_SAO_VARIAVEIS:
            erros.append(f"{onde}: {nome} no fonte e fora do catalogo (crates/phxclaw-config-runtime/src/agente/catalogo.rs)")
    for nome in sorted(no_catalogo):
        if nome not in vistos:
            erros.append(f"catalogo: {nome} ({no_catalogo[nome]['chave']}) nao aparece em fonte nenhum: configuracao que nao e lida mente")
    if a.listar:
        print("\n".join(soltas))
    print(f"variaveis no fonte: {len(vistos)}; chaves no catalogo: {len(no_catalogo)}; leituras soltas: {len(soltas)} (TETO {TETO})")
    if len(soltas) > TETO:
        erros.append(f"catraca: {len(soltas)} leituras soltas de PHXCLAW_* acima do TETO {TETO}: leitura nova entra por config::valor (--listar mostra onde)")
    elif len(soltas) < TETO:
        erros.append(f"catraca: {len(soltas)} leituras soltas, abaixo do TETO {TETO}: baixe o TETO para {len(soltas)} neste passo (so desce)")
    for e in erros:
        print(f"REPROVADO: {e}", file=sys.stderr)
    return 1 if erros else 0


if __name__ == "__main__":
    sys.exit(main())
