"""`ClienteApi` contra o `phxclaw servir` de verdade."""

from __future__ import annotations

from pathlib import Path

import pytest
from conftest import TOKEN, _Ollama

from phxclaw import ClienteApi, ErroApi


def test_cria_acompanha_e_le_o_resultado(servidor: str) -> None:
    api = ClienteApi(servidor, TOKEN)
    assert api.saude()
    id = api.criar_tarefa("escreva nota.txt", modelo="ollama:roteiro")
    t = api.aguardar(id, prazo=60)
    assert t["status"] == "completed", t
    assert t["answer"] == "feito: nota.txt"
    assert [p["tool"] for p in t["steps"] if p.get("tool") == "write_file"]
    assert t["usage"]["input_tokens"] > 0
    caminhos = [a["path"] for a in t["artifacts"]]
    assert "nota.txt" in caminhos, t
    assert api.artefato(id, "nota.txt") == b"ola do sdk python"
    # A listagem e o resumo: `steps` e a contagem.
    r = next(x for x in api.listar() if x["id"] == id)
    assert r["steps"] == len(t["steps"])
    # O modelo pedido foi o que chegou ao Ollama.
    assert any(p.get("model") == "roteiro" for p in _Ollama.pedidos)
    assert TOKEN not in repr(api)


def test_token_pelo_arquivo_da_pasta(
    servidor: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("PHXCLAW_API_TOKEN", raising=False)
    (tmp_path / "api.token").write_text(TOKEN + "\n")
    assert isinstance(ClienteApi(servidor, pasta=tmp_path).listar(), list)


def test_recusas_chegam_com_codigo_e_mensagem(
    servidor: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with pytest.raises(ErroApi) as e:
        ClienteApi(servidor, "token-errado-mas-comprido-o-bastante").listar()
    assert e.value.status == 401
    api = ClienteApi(servidor, TOKEN)
    with pytest.raises(ErroApi) as e:
        api.criar_tarefa("   ")
    assert e.value.status == 400 and "objective" in e.value.mensagem
    with pytest.raises(ErroApi) as e:
        api.criar_tarefa("x", modelo="sem-provedor")
    assert e.value.status == 400 and "provedor" in e.value.mensagem
    with pytest.raises(ErroApi) as e:
        api.tarefa("../../etc/passwd")
    assert e.value.status == 404
    monkeypatch.delenv("PHXCLAW_API_TOKEN", raising=False)
    monkeypatch.setenv("PHXCLAW_HOME", str(tmp_path / "vazia"))
    with pytest.raises(ValueError, match="sem token"):
        ClienteApi(servidor)
