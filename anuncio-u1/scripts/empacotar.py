# Gera saida/anuncio_u1_pacote.zip: o arquivo unico anuncio_u1.py na raiz e
# a pasta assets/ com EXATAMENTE os arquivos externos que o main() exige
# (montar.ASSETS_EXTERNOS) - nem 4k, nem .bak, nem previa, nem os WAV de
# saida do teste do som. E o que o cliente descompacta ao lado do .blend.
#
# Remonta o arquivo unico antes de zipar: o zip nunca carrega um
# anuncio_u1.py mais velho que os modulos (a licao do binario velho da
# bancada vale aqui). E confere que a lista de externos escrita DENTRO do
# arquivo unico e a mesma do montar.py - se divergissem, o main() exigiria
# um arquivo que o zip nao traz, ou o zip traria um que ninguem le.
#
# Uso: python3 scripts/empacotar.py            remonta, zipa, imprime tamanhos
#      python3 scripts/empacotar.py --provar   ...e PROVA no Blender que o zip
#                                              e autossuficiente e idempotente
#
# A PROVA (--provar), em duas passadas do Blender, sem xvfb (nao renderiza):
#   1. descompacta numa pasta nova do scratchpad; abre o Blender DE FABRICA
#      (Cube/Light/Camera), salva um .blend nessa pasta, e executa o texto
#      de anuncio_u1.py por exec() SEM __file__ e SEM o scripts/ do projeto
#      no sys.path, com a pasta de trabalho apontando para uma pasta vazia:
#      a unica pista para achar assets/ e o .blend salvo. Roda DUAS vezes na
#      mesma cena e compara as contagens (objetos, colecoes, materiais,
#      imagens, actions, sons, strips do VSE, malhas): iguais = idempotente.
#   2. reabre o anuncio_u1.blend gravado num processo limpo e le o que o
#      cliente vai renderizar: motor, resolucao, fps, quadros, AgX, motion
#      blur, container/codec de audio, faixas de som (empacotadas), imagens
#      (empacotadas), triangulos do u1.corpo, texturas da caixa.
# O que a prova espera esta em ESPERADO; qualquer diferenca sai listada e o
# processo termina com codigo 1.

import ast
import json
import os
import shutil
import subprocess
import sys
import zipfile

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
sys.path.insert(0, AQUI)
import montar  # noqa: E402

ZIP = os.path.join(RAIZ, "saida", "anuncio_u1_pacote.zip")
BLENDER = os.environ.get(
    "BLENDER", "/tmp/claude-0/-home-user-adrianoboller/857c9466-78dd-51a3-8b8e-9f13227d5fd4/scratchpad/blender/blender")
SCRATCH = os.environ.get(
    "SCRATCH", "/tmp/claude-0/-home-user-adrianoboller/857c9466-78dd-51a3-8b8e-9f13227d5fd4/scratchpad")

# O que o .blend gravado tem de ter (passada 2 da prova).
ESPERADO = {
    "engine": "BLENDER_EEVEE_NEXT",
    "resolucao": [1080, 1920],
    "fps": 30,
    "quadros": [1, 750],
    "view_transform": "AgX",
    "look": "AgX - Medium High Contrast",
    "motion_blur": True,
    "file_format": "FFMPEG",
    "ffmpeg_format": "MPEG4",
    "codec": "H264",
    "audio_codec": "AAC",
    "strips_som": ["som.efeitos", "som.trilha"],
    "imagens_pequenas": ["logo_engineprint.png", "u1.tela_boot", "u1.tela_ui"],   # a logo entra com o nome do arquivo (check_existing)
    "tris_u1_corpo": [250000, 400000],   # ~300 mil (o GLB inteiro tem 393.991)
}


def _mb(n):
    return "%.1f MB" % (n / 1048576.0)


def lista_do_arquivo_unico(caminho):
    """A tupla _ASSETS_EXTERNOS escrita no anuncio_u1.py, lida por ast (sem
    executar nada)."""
    with open(caminho, encoding="utf-8") as f:
        arvore = ast.parse(f.read())
    for no in arvore.body:
        if isinstance(no, ast.Assign) and any(getattr(a, "id", "") == "_ASSETS_EXTERNOS" for a in no.targets):
            return tuple(ast.literal_eval(no.value))
    raise SystemExit("[empacotar] o arquivo unico nao tem _ASSETS_EXTERNOS")


def empacotar():
    script = montar.montar()
    no_script = lista_do_arquivo_unico(script)
    if no_script != tuple(montar.ASSETS_EXTERNOS):
        raise SystemExit("[empacotar] lista de externos diverge: montar.py %s x arquivo unico %s"
                         % (montar.ASSETS_EXTERNOS, no_script))
    os.makedirs(os.path.dirname(ZIP), exist_ok=True)
    total = 0
    with zipfile.ZipFile(ZIP, "w", zipfile.ZIP_DEFLATED) as z:
        z.write(script, montar.NOME_SCRIPT)
        tam = os.path.getsize(script)
        total += tam
        print("[empacotar]  %-34s %12d bytes  %s" % (montar.NOME_SCRIPT, tam, _mb(tam)))
        for nome in montar.ASSETS_EXTERNOS:
            caminho = os.path.join(RAIZ, "assets", nome)
            z.write(caminho, "assets/" + nome)
            tam = os.path.getsize(caminho)
            total += tam
            print("[empacotar]  %-34s %12d bytes  %s" % ("assets/" + nome, tam, _mb(tam)))
    with zipfile.ZipFile(ZIP) as z:
        nomes = sorted(z.namelist())
    esperados = sorted([montar.NOME_SCRIPT] + ["assets/" + n for n in montar.ASSETS_EXTERNOS])
    if nomes != esperados:
        raise SystemExit("[empacotar] o zip nao tem exatamente o esperado: %s" % nomes)
    tam_zip = os.path.getsize(ZIP)
    print("[empacotar]  %-34s %12d bytes  %s  (%d arquivos, %s descompactados)"
          % (os.path.relpath(ZIP, RAIZ), tam_zip, _mb(tam_zip), len(nomes), _mb(total)))
    return ZIP


# ---------------------------------------------------------------- prova

# Passada 1: roda dentro do Blender de fabrica. argv depois de '--':
# pasta do pacote descompactado, caminho do JSON de saida.
PROVA_1 = r'''
import json, os, sys
import bpy

pasta, saida = sys.argv[sys.argv.index("--") + 1:][:2]

# Sem o projeto por perto: nenhum mod_*.py importavel e nenhum ja importado.
for p in list(sys.path):
    if os.path.isfile(os.path.join(p, "mod_coreografia.py")):
        sys.path.remove(p)
assert not [m for m in sys.modules if m.startswith("mod_")], "mod_* ja importado antes da prova"
assert not os.path.isdir(os.path.join(os.getcwd(), "assets")), "a pasta de trabalho tem assets/: a prova nao isolaria o .blend"

bpy.ops.wm.read_factory_settings()                    # Cube, Light, Camera
blend = os.path.join(pasta, "cena.blend")
bpy.ops.wm.save_as_mainfile(filepath=blend)
with open(os.path.join(pasta, "anuncio_u1.py"), encoding="utf-8") as f:
    fonte = f.read()


def contar():
    vse = bpy.context.scene.sequence_editor
    faixas = []
    if vse is not None:
        faixas = getattr(vse, "sequences", None)
        if faixas is None:
            faixas = vse.strips
    return {
        "objetos": len(bpy.data.objects),
        "colecoes": len(bpy.data.collections),
        "materiais": len(bpy.data.materials),
        "imagens": len(bpy.data.images),
        "actions": len(bpy.data.actions),
        "sons": len(bpy.data.sounds),
        "strips_vse": len(list(faixas)),
        "malhas": len(bpy.data.meshes),
        "luzes": len(bpy.data.lights),
        "cameras": len(bpy.data.cameras),
        "node_groups": len(bpy.data.node_groups),
        "worlds": len(bpy.data.worlds),
    }


def nomes():
    return {"objetos": sorted(o.name for o in bpy.data.objects),
            "imagens": sorted(i.name for i in bpy.data.images),
            "sons": sorted(s.name for s in bpy.data.sounds)}


def rodar():
    # Como a aba Scripting: __name__ == "__main__"; sem __file__ (o pior caso).
    ns = {"__name__": "__main__"}
    exec(compile(fonte, "anuncio_u1.py", "exec"), ns)
    return ns


res = {"blend": blend, "cwd": os.getcwd(), "rodadas": [], "nomes": []}
for i in (1, 2):
    print("\n[prova] ======== rodada %d ========" % i)
    ns = rodar()
    res["rodadas"].append(contar())
    res["nomes"].append(nomes())
    assert "__file__" not in ns, "o texto definiu __file__?"
res["sys_path_com_projeto"] = [p for p in sys.path if os.path.isfile(os.path.join(p, "mod_coreografia.py"))]
res["blend_gravado"] = os.path.join(pasta, "anuncio_u1.blend")
res["existe_blend_gravado"] = os.path.isfile(res["blend_gravado"])
res["tamanho_blend_gravado"] = os.path.getsize(res["blend_gravado"]) if res["existe_blend_gravado"] else 0
with open(saida, "w") as f:
    json.dump(res, f, indent=1)
print("[prova] rodadas:", res["rodadas"])
'''

# Passada 2: roda com o anuncio_u1.blend ABERTO (blender -b arquivo.blend -P).
PROVA_2 = r'''
import json, os, sys
import bpy

saida = sys.argv[sys.argv.index("--") + 1]
cena = bpy.context.scene
r = cena.render
vse = cena.sequence_editor
faixas = []
if vse is not None:
    faixas = getattr(vse, "sequences", None)
    if faixas is None:
        faixas = vse.strips


def tris(obj):
    return sum(len(p.vertices) - 2 for p in obj.data.polygons) if obj and obj.type == "MESH" else 0


def imagens_do_material(obj):
    out = []
    for slot in (obj.material_slots if obj else []):
        m = slot.material
        if m and m.use_nodes:
            for no in m.node_tree.nodes:
                if no.type == "TEX_IMAGE" and no.image:
                    out.append(no.image.name)
    return sorted(set(out))


u1_corpo = bpy.data.objects.get("u1.corpo")
caixa_corpo = bpy.data.objects.get("caixa.corpo")
etiqueta = bpy.data.objects.get("caixa.etiqueta")
info = {
    "engine": r.engine,
    "resolucao": [r.resolution_x, r.resolution_y],
    "porcentagem": r.resolution_percentage,
    "fps": r.fps, "fps_base": r.fps_base,
    "quadros": [cena.frame_start, cena.frame_end],
    "view_transform": cena.view_settings.view_transform,
    "look": cena.view_settings.look,
    "motion_blur": r.use_motion_blur,
    "motion_blur_shutter": getattr(r, "motion_blur_shutter", None),
    "motion_blur_position": getattr(r, "motion_blur_position", None),
    "amostras": cena.eevee.taa_render_samples,
    "file_format": r.image_settings.file_format,
    "ffmpeg_format": r.ffmpeg.format,
    "codec": r.ffmpeg.codec,
    "audio_codec": r.ffmpeg.audio_codec,
    "audio_bitrate": r.ffmpeg.audio_bitrate,
    "audio_mixrate": r.ffmpeg.audio_mixrate,
    "use_sequencer": r.use_sequencer,
    "filepath": r.filepath,
    "strips": [{"nome": s.name, "tipo": s.type, "canal": s.channel, "ini": s.frame_start,
                "dur": s.frame_final_duration, "volume": round(s.volume, 3),
                "som_empacotado": bool(s.sound and s.sound.packed_file)} for s in faixas],
    "imagens": [{"nome": i.name, "tam": list(i.size), "empacotada": i.packed_file is not None,
                 "source": i.source, "users": i.users} for i in bpy.data.images],
    "tris_u1_corpo": tris(u1_corpo),
    "objetos_u1": sorted(o.name for o in bpy.data.objects if o.name.startswith("u1.")),
    "imagens_u1_corpo": imagens_do_material(u1_corpo),
    "imagens_caixa_corpo": imagens_do_material(caixa_corpo),
    "imagens_etiqueta": imagens_do_material(etiqueta),
    "tris_caixa_corpo": tris(caixa_corpo),
    "tris_etiqueta": tris(etiqueta),
    "n_objetos": len(bpy.data.objects),
    "colecoes": sorted(c.name for c in bpy.data.collections),
}
with open(saida, "w") as f:
    json.dump(info, f, indent=1)
'''


def _blender(args, cwd, log):
    with open(log, "w") as f:
        proc = subprocess.run([BLENDER] + args, cwd=cwd, stdout=f, stderr=subprocess.STDOUT, timeout=590)
    return proc.returncode


def provar(caminho_zip):
    base = os.path.join(SCRATCH, "prova_pacote")
    shutil.rmtree(base, ignore_errors=True)
    pacote, prova, cwd = (os.path.join(base, n) for n in ("pacote", "prova", "cwd_vazio"))
    for d in (pacote, prova, cwd):
        os.makedirs(d)
    with zipfile.ZipFile(caminho_zip) as z:
        z.extractall(pacote)
    print("[prova] descompactado em", pacote)
    falhas = []

    # ---- passada 1: duas rodadas na cena de fabrica
    s1 = os.path.join(prova, "prova_1.py")
    with open(s1, "w") as f:
        f.write(PROVA_1)
    j1 = os.path.join(prova, "rodadas.json")
    log1 = os.path.join(prova, "prova_1.log")
    codigo = _blender(["-b", "--python", s1, "--", pacote, j1], cwd, log1)
    print("[prova] passada 1: blender saiu com %d, log em %s" % (codigo, log1))
    if codigo != 0 or not os.path.isfile(j1):
        with open(log1) as f:
            print(f.read()[-4000:])
        raise SystemExit("[prova] passada 1 falhou")
    with open(j1) as f:
        r1 = json.load(f)
    a, b = r1["rodadas"]
    print("[prova] contagens:  %-12s %8s %8s" % ("", "rodada 1", "rodada 2"))
    for k in a:
        marca = "" if a[k] == b[k] else "   <-- DIFERENTE"
        print("[prova]             %-12s %8d %8d%s" % (k, a[k], b[k], marca))
        if a[k] != b[k]:
            falhas.append("contagem de %s mudou entre as rodadas (%d -> %d)" % (k, a[k], b[k]))
    for k in ("objetos", "imagens", "sons"):
        if r1["nomes"][0][k] != r1["nomes"][1][k]:
            so1 = sorted(set(r1["nomes"][0][k]) - set(r1["nomes"][1][k]))
            so2 = sorted(set(r1["nomes"][1][k]) - set(r1["nomes"][0][k]))
            falhas.append("nomes de %s diferem: so na 1: %s; so na 2: %s" % (k, so1, so2))
    if r1["sys_path_com_projeto"]:
        falhas.append("o sys.path tinha o projeto: %s" % r1["sys_path_com_projeto"])
    if not r1["existe_blend_gravado"]:
        falhas.append("anuncio_u1.blend nao foi gravado")
    print("[prova] .blend gravado: %s (%s)" % (r1["blend_gravado"], _mb(r1["tamanho_blend_gravado"])))
    with open(log1) as f:
        for linha in f:
            if "[anuncio] assets externos em" in linha or "[som] " in linha and "cues" in linha:
                print("[prova] " + linha.rstrip())

    # ---- passada 2: o .blend gravado, num processo limpo
    s2 = os.path.join(prova, "prova_2.py")
    with open(s2, "w") as f:
        f.write(PROVA_2)
    j2 = os.path.join(prova, "blend.json")
    log2 = os.path.join(prova, "prova_2.log")
    codigo = _blender(["-b", r1["blend_gravado"], "--python", s2, "--", j2], cwd, log2)
    print("[prova] passada 2: blender saiu com %d, log em %s" % (codigo, log2))
    if codigo != 0 or not os.path.isfile(j2):
        with open(log2) as f:
            print(f.read()[-4000:])
        raise SystemExit("[prova] passada 2 falhou")
    with open(j2) as f:
        info = json.load(f)

    def esperar(chave, valor, ok):
        estado = "ok" if ok else "FALHA"
        print("[prova]   %-6s %-18s %s" % (estado, chave, valor))
        if not ok:
            falhas.append("%s = %r" % (chave, valor))

    print("[prova] .blend gravado:")
    for k in ("engine", "resolucao", "fps", "quadros", "view_transform", "look", "motion_blur",
              "file_format", "ffmpeg_format", "codec", "audio_codec"):
        esperar(k, info[k], info[k] == ESPERADO[k])
    esperar("motion_blur_shutter", info["motion_blur_shutter"], info["motion_blur_shutter"] is not None)
    esperar("audio_mixrate", info["audio_mixrate"], info["audio_mixrate"] == 48000)
    esperar("amostras", info["amostras"], info["amostras"] > 0)
    esperar("filepath", info["filepath"], info["filepath"].endswith("anuncio_u1.mp4"))
    strips = sorted(s["nome"] for s in info["strips"] if s["tipo"] == "SOUND")
    esperar("strips_som", strips, strips == ESPERADO["strips_som"])
    for s in info["strips"]:
        esperar("  " + s["nome"], "canal %d, quadros %d..%d, volume %.3f, %s" % (
            s["canal"], s["ini"], s["ini"] + s["dur"] - 1, s["volume"],
            "empacotado" if s["som_empacotado"] else "NAO empacotado"),
            s["som_empacotado"] and s["dur"] == 750)
    imgs = {i["nome"]: i for i in info["imagens"]}
    for nome in ESPERADO["imagens_pequenas"]:
        i = imgs.get(nome)
        esperar("imagem " + nome, "%s %s" % (i["tam"], "empacotada" if i["empacotada"] else "NAO empacotada") if i else "AUSENTE",
                bool(i and i["empacotada"]))
    lo, hi = ESPERADO["tris_u1_corpo"]
    esperar("tris_u1_corpo", info["tris_u1_corpo"], lo <= info["tris_u1_corpo"] <= hi)
    esperar("imagens_u1_corpo", info["imagens_u1_corpo"], len(info["imagens_u1_corpo"]) >= 1
            and all(imgs[n]["empacotada"] for n in info["imagens_u1_corpo"]))
    esperar("imagens_caixa_corpo", info["imagens_caixa_corpo"], len(info["imagens_caixa_corpo"]) == 3
            and all(imgs[n]["empacotada"] for n in info["imagens_caixa_corpo"]))
    esperar("imagens_etiqueta", info["imagens_etiqueta"], len(info["imagens_etiqueta"]) == 3
            and all(imgs[n]["empacotada"] for n in info["imagens_etiqueta"]))
    esperar("tris_caixa/etiqueta", "%d / %d" % (info["tris_caixa_corpo"], info["tris_etiqueta"]),
            info["tris_caixa_corpo"] > 0 and info["tris_etiqueta"] > 0)
    nao_emp = [i["nome"] for i in info["imagens"] if not i["empacotada"] and i["source"] == "FILE" and i["users"] > 0]
    esperar("imagens de arquivo sem pack", nao_emp, not nao_emp)
    print("[prova] objetos u1.*: %s" % ", ".join(info["objetos_u1"]))
    print("[prova] colecoes: %s" % ", ".join(info["colecoes"]))
    print("[prova] resultado:", "TUDO OK" if not falhas else "%d FALHA(S):\n  " % len(falhas) + "\n  ".join(falhas))
    return not falhas


if __name__ == "__main__":
    caminho = empacotar()
    if "--provar" in sys.argv:
        sys.exit(0 if provar(caminho) else 1)
