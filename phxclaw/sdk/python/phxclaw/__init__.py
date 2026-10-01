"""SDK Python do PhxClaw.

- `ClienteApi`: a API HTTP de tarefas (`phxclaw servir`) -- criar, acompanhar, aprovar,
  cancelar e ler o resultado e os artefatos.
- `ClienteMcp`: as ferramentas do agente pelo `phxclaw mcp-serve`, por stdio.

So a biblioteca padrao. O modulo nativo (`phxclaw_nativo`, em `sdk/python-nativo`) e
outro pacote, opcional: quem so fala com o servidor nao precisa compilar Rust.
"""

from phxclaw.api import (
    PARADOS,
    Artefato,
    ClienteApi,
    ErroApi,
    Passo,
    ResumoTarefa,
    Tarefa,
    Uso,
    token_do_ambiente,
)
from phxclaw.mcp import ClienteMcp, ErroMcp, ResultadoFerramenta

__all__ = [
    "PARADOS",
    "Artefato",
    "ClienteApi",
    "ClienteMcp",
    "ErroApi",
    "ErroMcp",
    "Passo",
    "ResultadoFerramenta",
    "ResumoTarefa",
    "Tarefa",
    "Uso",
    "token_do_ambiente",
]
__version__ = "0.70.0"
