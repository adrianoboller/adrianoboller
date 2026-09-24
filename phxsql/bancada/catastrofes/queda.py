#!/usr/bin/env python3
"""A queda de energia, contra o sistema operacional -- pedido 533.

    python3 queda.py DIR_DA_TABELA TABELA nada|paginas

Chamado pelo `prova.sh`, cenario 533, com o ext4 montado num espaco de
montagem privado. Decide O QUE chegou ao disco antes da queda e derruba o
sistema de arquivos sem descarregar o resto:

* `nada`    -- o `.reg` chega inteiro; do `.ndx`, NADA que o processo nao
               tenha sincronizado ele mesmo. E a queda mais provavel do C4' do
               papel J: o nucleo devolve cada inode ao disco por conta propria,
               e o `.reg` sujou antes do `.ndx`.
* `paginas` -- o `.reg` chega inteiro, e do `.ndx` chegam as paginas a partir
               da 1 -- a pagina 0, o cabecalho com o byte 52, fica no cache.
               E o C4 do papel C: «as paginas novas chegam, a pagina 0 nao».

Como: `sync_file_range` (espera antes, escreve, espera depois) nos intervalos
escolhidos; um `fsync` do `.reg`, para o diario do ext4 ter cometido o tamanho
novo dele (o cabecalho do `.ndx` e um bloco que ja existia, entao nenhum commit
o leva); e `FS_IOC_SHUTDOWN` com `EXT4_GOING_FLAGS_NOLOGFLUSH`, que marca o
sistema de arquivos como caido e aborta o diario: o que ainda estava sujo no
cache nao chega mais ao disco. E o `godown` do xfstests, o mesmo recurso que
eles usam para emular a queda.

O `fsync` do `.reg` e garantia do cenario, e NAO a causa dele -- medido em
24/09/2026, 1 rodada de cada, com o binario de antes do 533: sem ele, e ate
trocando o `NOLOGFLUSH` pelo `LOGFLUSH`, o disco guardou o MESMO `.reg` novo
(2.030.960 bytes, 35.000 linhas) e o `.ndx` do ultimo fecho (byte 52 = 0). A
hipotese de que o `LOGFLUSH` abortaria o commit (o `data=ordered` escreveria
os dados do inode depois do `shutdown` e levaria `EIO`) morreu medida --
`docs/cognicao/cognicao_queda-de-energia-pelo-shutdown-do-ext4_20260924_2010.md`.

O que isto NAO emula: o cache volatil do proprio disco reordenando escritas.
Aqui a ordem e a do roteiro; o que se prova e que, com o conserto, o 1 ja
esta no disco ANTES de qualquer pagina -- porque o `fdatasync` da subida o
levou --, qualquer que seja a ordem que o resto chegaria.

Sai 0 quando derrubou; 2 quando o nucleo nao aceita (sem `CAP_SYS_ADMIN`, ou
sistema de arquivos sem o ioctl) -- e o roteiro diz NAO PROVADO.
"""
import ctypes
import fcntl
import os
import struct
import sys

# _IOR('X', 125, __u32): (2 << 30) | (4 << 16) | (ord('X') << 8) | 125
FS_IOC_SHUTDOWN = 0x8004587D
EXT4_GOING_FLAGS_NOLOGFLUSH = 2
SYNC_FILE_RANGE_WAIT_BEFORE = 1
SYNC_FILE_RANGE_WRITE = 2
SYNC_FILE_RANGE_WAIT_AFTER = 4
PAGINA_DO_NDX = 4096


def descer(caminho, de):
    """Leva ao disco o intervalo [de, fim) de `caminho`, e so ele."""
    libc = ctypes.CDLL(None, use_errno=True)
    sfr = libc.sync_file_range
    sfr.argtypes = [ctypes.c_int, ctypes.c_longlong, ctypes.c_longlong, ctypes.c_uint]
    fd = os.open(caminho, os.O_RDONLY)
    try:
        tamanho = os.fstat(fd).st_size
        if tamanho > de:
            flags = (SYNC_FILE_RANGE_WAIT_BEFORE | SYNC_FILE_RANGE_WRITE
                     | SYNC_FILE_RANGE_WAIT_AFTER)
            if sfr(fd, de, tamanho - de, flags) != 0:
                raise OSError(ctypes.get_errno(), f"sync_file_range {caminho}")
        return tamanho
    finally:
        os.close(fd)


def main():
    if len(sys.argv) != 4 or sys.argv[3] not in ("nada", "paginas"):
        print(__doc__)
        return 2
    dir_, tabela, modo = sys.argv[1:4]
    reg = os.path.join(dir_, f"{tabela}.reg")
    ndx = os.path.join(dir_, f"{tabela}.ndx")
    t_reg = descer(reg, 0)
    t_ndx = os.path.getsize(ndx)
    if modo == "paginas":
        descer(ndx, PAGINA_DO_NDX)
    # O commit do diario: o `.reg` cresceu, e o `fsync` dele garante o
    # tamanho novo cometido (medido: sem ele o disco guardou o mesmo -- ver o
    # cabecalho). Nada do `.ndx` que ainda estava sujo entra, porque o ext4 em
    # `data=ordered` so escreve no commit o que teve bloco ALOCADO -- e a
    # pagina 0 ja tinha.
    fd = os.open(reg, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    fd = os.open(dir_, os.O_RDONLY)
    try:
        fcntl.ioctl(fd, FS_IOC_SHUTDOWN, struct.pack("I", EXT4_GOING_FLAGS_NOLOGFLUSH))
    except OSError as e:
        print(f"queda=NAO {e}")
        return 2
    finally:
        os.close(fd)
    print(f"queda={modo} reg={t_reg} ndx={t_ndx}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
