"""O modulo nativo importado de verdade (a roda do maturin), contra o nucleo do UI-IR."""

from __future__ import annotations

import pytest

import phxclaw_nativo as nativo

DDL = """
CREATE TABLE clientes (
    id SERIAL PRIMARY KEY,
    nome VARCHAR(80) NOT NULL,
    situacao VARCHAR(10) CHECK (situacao IN ('ativo', 'inativo'))
);
CREATE TABLE pedidos (
    id SERIAL PRIMARY KEY,
    cliente_id INTEGER NOT NULL REFERENCES clientes(id),
    total NUMERIC(12, 2) NOT NULL,
    emitido_em DATE NOT NULL
);
"""


def test_sql_vira_ui_ir_e_html() -> None:
    app, avisos = nativo.ui_ir("Loja", DDL)
    assert isinstance(app, dict) and isinstance(avisos, list)
    assert app["name"] == "Loja"
    nomes = [e["name"] for e in app["entities"]]
    assert nomes == ["clientes", "pedidos"], nomes
    pedidos = app["entities"][1]
    campos = {c["name"]: c for c in pedidos["fields"]}
    # A chave estrangeira vira busca na outra tabela; o CHECK ... IN vira lista fixa.
    assert campos["cliente_id"]["widget"]["kind"] == "lookup", campos["cliente_id"]
    assert campos["total"]["required"] is True
    situacao = next(c for c in app["entities"][0]["fields"] if c["name"] == "situacao")
    assert situacao["widget"] == {"kind": "select", "options": ["ativo", "inativo"]}

    pagina = nativo.html("Loja", DDL)
    assert pagina.lstrip().lower().startswith("<!doctype html")
    assert "Loja" in pagina


def test_normalizar_tolera_ocr() -> None:
    assert nativo.normalizar("Situação*") == "situacao"
    assert nativo.normalizar("  CÓDIGO do Cliente:") == "codigodocliente"


def test_recusas() -> None:
    with pytest.raises(ValueError, match="nenhuma tabela"):
        nativo.ui_ir("Loja", "isto nao e sql")
    with pytest.raises(ValueError, match="nome"):
        nativo.ui_ir("  ", DDL)
    with pytest.raises(ValueError, match="nenhuma tabela"):
        nativo.html("Loja", "")
    with pytest.raises(TypeError):
        nativo.normalizar(42)  # type: ignore[arg-type]
