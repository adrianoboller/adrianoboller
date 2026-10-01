"""Cliente MCP por stdio do `phxclaw mcp-serve`.

O servidor fala JSON-RPC 2.0, uma mensagem por linha, no stdout dele -- e nada mais se
escreve ali; o stderr e do operador. A leitura corre numa thread com fila para que todo
pedido tenha prazo: um servidor travado vira `TimeoutError`, nunca um `readline` eterno.
"""

from __future__ import annotations

import json
import os
import queue
import subprocess
import threading
from collections.abc import Mapping, Sequence
from typing import IO, Any, cast

#: Versao do protocolo que o cliente propoe; o servidor responde com a que negociou.
VERSAO_DO_PROTOCOLO = "2025-06-18"


class ErroMcp(Exception):
    """Erro JSON-RPC devolvido pelo servidor (metodo desconhecido, parametro invalido)."""

    def __init__(self, codigo: int, mensagem: str) -> None:
        super().__init__(f"MCP {codigo}: {mensagem}")
        self.codigo = codigo
        self.mensagem = mensagem


class ResultadoFerramenta:
    """Resultado de `tools/call`: o texto e se a ferramenta recusou ou falhou.

    `erro=True` nao e excecao de proposito: e a resposta normal de uma ferramenta negada
    pela politica ou com argumento ruim, e quem chama decide o que fazer com ela.
    """

    def __init__(self, bruto: Mapping[str, Any]) -> None:
        self.bruto = dict(bruto)
        self.erro = bool(bruto.get("isError", False))
        partes = bruto.get("content") or []
        self.texto = "\n".join(
            str(p.get("text", ""))
            for p in partes
            if isinstance(p, dict) and p.get("type") == "text"
        )

    def json(self) -> Any:
        """O texto como JSON -- as ferramentas de projeto (`python_project`,
        `rust_project`) devolvem diagnosticos estruturados assim."""
        return json.loads(self.texto)

    def __repr__(self) -> str:
        return f"ResultadoFerramenta(erro={self.erro}, texto={self.texto[:80]!r})"


class ClienteMcp:
    """Sobe `phxclaw mcp-serve` e conversa com ele.

    Use como gerenciador de contexto: a saida fecha o stdin do servidor, que termina
    soltando o que as ferramentas seguraram para a sessao.
    """

    def __init__(
        self,
        comando: Sequence[str] = ("phxclaw", "mcp-serve"),
        *,
        env: Mapping[str, str] | None = None,
        cwd: str | os.PathLike[str] | None = None,
        prazo: float = 60.0,
    ) -> None:
        self.prazo = prazo
        self._proximo = 0
        self._fila: queue.Queue[dict[str, Any] | None] = queue.Queue()
        self.servidor: dict[str, Any] = {}
        self._p = subprocess.Popen(
            list(comando),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=None if env is None else dict(env),
            cwd=cwd,
        )
        self._stderr: list[str] = []
        threading.Thread(target=self._ler, daemon=True).start()
        threading.Thread(target=self._ler_stderr, daemon=True).start()

    # ------------------------------------------------------------------ fio

    def _ler(self) -> None:
        saida = cast(IO[bytes], self._p.stdout)
        for linha in saida:
            if not linha.strip():
                continue
            try:
                self._fila.put(json.loads(linha))
            except ValueError:
                # Linha que nao e JSON no fio e defeito do servidor; fica no stderr colhido.
                self._stderr.append(f"[stdout nao-JSON] {linha!r}")
        self._fila.put(None)

    def _ler_stderr(self) -> None:
        for linha in cast(IO[bytes], self._p.stderr):
            self._stderr.append(linha.decode(errors="replace").rstrip())

    @property
    def stderr(self) -> str:
        """O que o servidor escreveu no stderr (avisos de configuracao, por exemplo)."""
        return "\n".join(self._stderr)

    def _enviar(self, msg: dict[str, Any]) -> None:
        entrada = cast(IO[bytes], self._p.stdin)
        entrada.write(json.dumps(msg).encode() + b"\n")
        entrada.flush()

    def pedir(self, metodo: str, parametros: Mapping[str, Any] | None = None) -> Any:
        """Um pedido JSON-RPC; devolve o `result` ou levanta `ErroMcp`."""
        self._proximo += 1
        id = self._proximo
        msg: dict[str, Any] = {"jsonrpc": "2.0", "id": id, "method": metodo}
        if parametros is not None:
            msg["params"] = dict(parametros)
        self._enviar(msg)
        while True:
            try:
                r = self._fila.get(timeout=self.prazo)
            except queue.Empty:
                raise TimeoutError(f"{metodo}: sem resposta em {self.prazo}s") from None
            if r is None:
                raise ConnectionError(f"servidor MCP terminou: {self.stderr[-2000:]}")
            if r.get("id") != id:
                continue  # notificacao do servidor ou resposta atrasada de outro pedido
            if "error" in r:
                e = r["error"]
                raise ErroMcp(int(e.get("code", 0)), str(e.get("message", "")))
            return r.get("result")

    def notificar(self, metodo: str, parametros: Mapping[str, Any] | None = None) -> None:
        msg: dict[str, Any] = {"jsonrpc": "2.0", "method": metodo}
        if parametros is not None:
            msg["params"] = dict(parametros)
        self._enviar(msg)

    # ------------------------------------------------------------------ protocolo

    def inicializar(self) -> dict[str, Any]:
        r = self.pedir(
            "initialize",
            {
                "protocolVersion": VERSAO_DO_PROTOCOLO,
                "capabilities": {},
                "clientInfo": {"name": "phxclaw-sdk-python", "version": "0.70.0"},
            },
        )
        self.servidor = dict(r)
        self.notificar("notifications/initialized")
        return self.servidor

    def ferramentas(self) -> list[dict[str, Any]]:
        """`tools/list`: so as ferramentas que a politica do servidor concede."""
        r = self.pedir("tools/list", {})
        return list(r.get("tools", []))

    def chamar(self, nome: str, argumentos: Mapping[str, Any] | None = None) -> ResultadoFerramenta:
        r = self.pedir("tools/call", {"name": nome, "arguments": dict(argumentos or {})})
        return ResultadoFerramenta(r)

    # ------------------------------------------------------------------ ciclo de vida

    def fechar(self, prazo: float = 10.0) -> int | None:
        if self._p.stdin and not self._p.stdin.closed:
            self._p.stdin.close()
        try:
            return self._p.wait(timeout=prazo)
        except subprocess.TimeoutExpired:
            self._p.kill()
            return self._p.wait()

    def __enter__(self) -> ClienteMcp:
        self.inicializar()
        return self

    def __exit__(self, *_: object) -> None:
        self.fechar()
