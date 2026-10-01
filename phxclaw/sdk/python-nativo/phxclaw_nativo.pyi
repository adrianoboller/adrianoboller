"""Tipos do modulo nativo (o maturin poe este arquivo na roda, ao lado do .so)."""

from typing import Any

__version__: str

def ui_ir(nome: str, sql: str) -> tuple[dict[str, Any], list[str]]:
    """DDL SQL -> (UI-IR, avisos). ValueError se nenhuma tabela for reconhecida."""

def html(nome: str, sql: str) -> str:
    """DDL SQL -> HTML da aplicacao. ValueError se nenhuma tabela for reconhecida."""

def normalizar(texto: str) -> str:
    """Sem acento, sem caixa, so letras e digitos (comparacao tolerante a OCR)."""
