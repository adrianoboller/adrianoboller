"""`ClienteMcp` contra o `phxclaw mcp-serve` de verdade.

O caso que fecha a integracao: um cliente Python pede, pelo MCP, que o agente rode o
mypy num projeto Python -- e recebe o erro de tipo como dado, com arquivo e linha.
"""

from __future__ import annotations

from pathlib import Path

import pytest
from conftest import ambiente, binario

from phxclaw import ClienteMcp, ErroMcp


def cliente(trabalho: Path, pasta: Path, capacidades: str) -> ClienteMcp:
    return ClienteMcp(
        [str(binario()), "mcp-serve", "--trabalho", str(trabalho), "--pasta", str(pasta)],
        env=ambiente(PHXCLAW_CAPACIDADES=capacidades),
        prazo=120,
    )


def test_lista_chama_e_devolve_diagnostico_do_python(tmp_path: Path) -> None:
    trabalho = tmp_path / "trabalho"
    with cliente(trabalho, tmp_path / "agente", "fs.read,fs.write,shell.exec") as mcp:
        assert mcp.servidor["serverInfo"]["name"]
        nomes = {f["name"] for f in mcp.ferramentas()}
        assert {"write_file", "read_file"} <= nomes, nomes
        if "python_project" not in nomes:
            pytest.skip(f"python_project nao registrada (bwrap/interpretador?): {mcp.stderr}")

        r = mcp.chamar(
            "write_file",
            {"path": "calc.py", "content": "def dobro(x: int) -> int:\n    return str(x)\n"},
        )
        assert not r.erro, r
        assert (trabalho / "calc.py").is_file()

        r = mcp.chamar("python_project", {"action": "typecheck"})
        assert not r.erro, r
        v = r.json()
        assert v["sucesso"] is False, v
        d = next(d for d in v["diagnosticos"] if d["codigo"] == "return-value")
        assert (d["arquivo"], d["linha"]) == ("calc.py", 2), v

        r = mcp.chamar("read_file", {"path": "calc.py"})
        assert "dobro" in r.texto


def test_recusas_do_protocolo_e_da_politica(tmp_path: Path) -> None:
    # Sem shell.exec concedida: python_project nem aparece, e chama-la e recusa.
    with cliente(tmp_path / "t", tmp_path / "agente", "fs.read") as mcp:
        nomes = {f["name"] for f in mcp.ferramentas()}
        assert "python_project" not in nomes and "write_file" not in nomes, nomes
        r = mcp.chamar("python_project", {"action": "test"})
        assert r.erro, r
        r = mcp.chamar("write_file", {"path": "x.txt", "content": "x"})
        assert r.erro and not (tmp_path / "t" / "x.txt").exists(), r
        with pytest.raises(ErroMcp) as e:
            mcp.pedir("metodo/inexistente", {})
        assert e.value.codigo == -32601
        with pytest.raises(ErroMcp):
            mcp.pedir("tools/call", {"arguments": {}})
