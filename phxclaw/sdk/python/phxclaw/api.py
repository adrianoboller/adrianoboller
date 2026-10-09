"""Cliente da API HTTP de tarefas do PhxClaw (`phxclaw servir`).

So `urllib`: o SDK nao arrasta dependencia nenhuma. Os tipos abaixo sao `TypedDict` sobre
o JSON que a API ja serializa -- as chaves sao as do servidor, sem traducao, para o
formato continuar tendo um dono so (o `phxclaw-agent-core`).
"""

from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Literal, TypedDict, cast

Estado = Literal[
    "pending",
    "awaiting_approval",
    "awaiting_input",
    "running",
    "completed",
    "failed",
    "cancelled",
    "budget_exceeded",
]

#: Estados em que a tarefa nao anda mais sozinha. `awaiting_approval` entra porque
#: esperar por ele e esperar por uma pessoa: o `aguardar` devolve e quem chama decide.
PARADOS: frozenset[str] = frozenset(
    {"completed", "failed", "cancelled", "budget_exceeded", "awaiting_approval"}
)


class Uso(TypedDict):
    input_tokens: int
    output_tokens: int


class Artefato(TypedDict):
    path: str
    media_type: str
    bytes: int
    sha256: str


class Passo(TypedDict, total=False):
    n: int
    at: str
    kind: str
    tool: str
    arguments: Any
    outcome: str
    summary: str


class Tarefa(TypedDict):
    id: str
    objective: str
    model: str
    status: Estado
    created_at: str
    updated_at: str
    plan: list[str]
    steps: list[Passo]
    answer: str | None
    artifacts: list[Artefato]
    usage: Uso
    error: str | None
    parent: str | None
    webhook: str | None


class ResumoTarefa(TypedDict):
    """O que a listagem devolve: `steps` e a contagem, nao a lista."""

    id: str
    objective: str
    status: Estado
    model: str
    created_at: str
    updated_at: str
    answer: str | None
    error: str | None
    steps: int
    artifacts: list[Artefato]
    usage: Uso
    parent: str | None


class ErroApi(Exception):
    """Recusa da API com o codigo HTTP e a mensagem dela.

    `retry_after` vem no 429 do limite de criacao: e o servidor dizendo quando tentar.
    """

    def __init__(self, status: int, mensagem: str, retry_after: int | None = None) -> None:
        super().__init__(f"HTTP {status}: {mensagem}")
        self.status = status
        self.mensagem = mensagem
        self.retry_after = retry_after


def token_do_ambiente(pasta: str | os.PathLike[str] | None = None) -> str | None:
    """O mesmo caminho do servidor: `PHXCLAW_API_TOKEN`, senao o `api.token` da pasta
    (`--pasta`, `PHXCLAW_HOME` ou `var/agente`) que o `servir` grava na primeira vez."""
    t = os.environ.get("PHXCLAW_API_TOKEN")
    if t:
        return t
    raiz = Path(pasta or os.environ.get("PHXCLAW_HOME") or "var/agente")
    try:
        return (raiz / "api.token").read_text().strip() or None
    except OSError:
        return None


class ClienteApi:
    """Cria, acompanha, aprova, cancela e le tarefas do agente."""

    def __init__(
        self,
        url: str = "http://127.0.0.1:8787",
        token: str | None = None,
        *,
        pasta: str | os.PathLike[str] | None = None,
        prazo: float = 30.0,
    ) -> None:
        self.url = url.rstrip("/")
        token = token or token_do_ambiente(pasta)
        if not token:
            raise ValueError(
                "sem token: passe token=, defina PHXCLAW_API_TOKEN ou aponte pasta= para "
                "onde o `phxclaw servir` gravou o api.token"
            )
        self._token = token
        self.prazo = prazo
        # Servidor em loopback nao passa por proxy de ambiente: o Bearer iria para um
        # terceiro que nada tem a ver com uma conversa entre dois processos da maquina.
        host = urllib.parse.urlsplit(self.url).hostname or ""
        handlers = (
            [urllib.request.ProxyHandler({})] if host in ("127.0.0.1", "::1", "localhost") else []
        )
        self._abrir = urllib.request.build_opener(*handlers).open

    def __repr__(self) -> str:
        # O token nunca aparece em repr, log ou traceback.
        return f"ClienteApi(url={self.url!r})"

    # ------------------------------------------------------------------ transporte

    def _pedir(self, metodo: str, caminho: str, corpo: Any = None, *, cru: bool = False) -> Any:
        dados = None if corpo is None else json.dumps(corpo).encode()
        req = urllib.request.Request(self.url + caminho, data=dados, method=metodo)
        req.add_header("Authorization", f"Bearer {self._token}")
        if dados is not None:
            req.add_header("Content-Type", "application/json")
        try:
            with self._abrir(req, timeout=self.prazo) as r:
                bruto = r.read()
        except urllib.error.HTTPError as e:
            texto = e.read().decode(errors="replace")
            try:
                v = json.loads(texto)
                msg = str(v.get("error", texto))
                retry = v.get("retry_after")
            except (ValueError, AttributeError):
                msg, retry = texto, None
            raise ErroApi(e.code, msg, retry) from None
        if cru:
            return bruto
        return json.loads(bruto) if bruto else None

    # ------------------------------------------------------------------ rotas

    def saude(self) -> bool:
        return bool(self._pedir("GET", "/health").get("ok"))

    def criar_tarefa(
        self,
        objetivo: str,
        *,
        modelo: str | None = None,
        plano_primeiro: bool = False,
        webhook: str | None = None,
        verificar: str | None = None,
        saida_esquema: dict[str, Any] | None = None,
    ) -> str:
        """Cria e devolve o id; a tarefa roda em segundo plano no servidor.

        `verificar`: comando que confere o fim no sandbox da tarefa (codigo != 0 recusa a
        resposta; o servidor exige a capacidade shell.exec). `saida_esquema`: esquema JSON
        da resposta final, conferido pelo mesmo validador dos argumentos das ferramentas.
        """
        corpo: dict[str, Any] = {"objective": objetivo, "plan_first": plano_primeiro}
        if modelo is not None:
            corpo["model"] = modelo
        if webhook is not None:
            corpo["webhook"] = webhook
        if verificar is not None:
            corpo["verificar"] = verificar
        if saida_esquema is not None:
            corpo["saida_esquema"] = saida_esquema
        return str(self._pedir("POST", "/v1/tasks", corpo)["id"])

    def tarefa(self, id: str) -> Tarefa:
        return cast(Tarefa, self._pedir("GET", f"/v1/tasks/{_id(id)}"))

    def listar(self) -> list[ResumoTarefa]:
        return cast(list[ResumoTarefa], self._pedir("GET", "/v1/tasks"))

    def aguardar(self, id: str, *, prazo: float = 120.0, intervalo: float = 0.2) -> Tarefa:
        """Sonda ate a tarefa parar (ver `PARADOS`). Consultar nao gasta ficha do servidor."""
        fim = time.monotonic() + prazo
        while True:
            t = self.tarefa(id)
            if t["status"] in PARADOS:
                return t
            if time.monotonic() >= fim:
                raise TimeoutError(f"tarefa {id} ainda em {t['status']} depois de {prazo}s")
            time.sleep(intervalo)

    def executar(self, objetivo: str, *, modelo: str | None = None, prazo: float = 120.0) -> Tarefa:
        """Atalho: cria e espera."""
        return self.aguardar(self.criar_tarefa(objetivo, modelo=modelo), prazo=prazo)

    def editar_plano(self, id: str, passos: list[str]) -> Tarefa:
        return cast(Tarefa, self._pedir("POST", f"/v1/tasks/{_id(id)}/plan", {"steps": passos}))

    def aprovar(self, id: str) -> None:
        self._pedir("POST", f"/v1/tasks/{_id(id)}/approve", {})

    def cancelar(self, id: str) -> str:
        return str(self._pedir("POST", f"/v1/tasks/{_id(id)}/cancel", {})["status"])

    def artefato(self, id: str, caminho: str) -> bytes:
        rota = f"/v1/tasks/{_id(id)}/artifacts/{urllib.parse.quote(caminho)}"
        return cast(bytes, self._pedir("GET", rota, cru=True))

    def agendar(
        self,
        nome: str,
        objetivo: str,
        *,
        cron: str | None = None,
        a_cada_segundos: int | None = None,
    ) -> dict[str, Any]:
        if (cron is None) == (a_cada_segundos is None):
            raise ValueError("informe cron OU a_cada_segundos")
        corpo: dict[str, Any] = {"name": nome, "objective": objetivo}
        if cron is not None:
            corpo["cron"] = cron
        else:
            corpo["every_seconds"] = a_cada_segundos
        return cast(dict[str, Any], self._pedir("POST", "/v1/schedules", corpo))


def _id(id: str) -> str:
    # O servidor ja filtra o id; quotar aqui impede que um id com `/` mude a ROTA pedida.
    return urllib.parse.quote(id, safe="")
