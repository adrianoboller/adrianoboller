# Diagnostico da cena do Snapmaker U1 aberta no Blender.
#
# Roda no Blender do Adriano e descreve o que existe na cena, porque o
# container que escreve o anuncio nao enxerga a maquina dele: o modelo nunca
# viaja, so este relatorio. O que o relatorio responde decide o anuncio -
# peca separada permite close e troca de cabecote animada, malha unica nao.
#
# Como usar: aba Scripting > Novo > cola isto > Executar (Alt+P).
# O caminho do relatorio aparece no fim do Console.

import bpy
import json
import os
import sys
from collections import Counter
from mathutils import Vector

# Limite do relatorio em texto. O JSON leva a cena inteira; o texto so os
# maiores, porque um STL de impressora pode trazer centenas de objetos e
# ninguem le isso colado numa conversa.
MAIORES_NO_TEXTO = 30


def pasta_de_saida():
    # A pasta do .blend e o lugar mais achavel; se o arquivo nunca foi salvo,
    # cai na home, que sempre existe e sempre e gravavel.
    if bpy.data.filepath:
        return os.path.dirname(bpy.data.filepath)
    return os.path.expanduser("~")


def mm(valor_em_unidades, escala):
    # As dimensoes do U1 estao em milimetros na ficha tecnica (584x499x730),
    # entao o relatorio fala milimetro para dar para conferir de bate-pronto.
    return round(valor_em_unidades * escala * 1000.0, 2)


# Ficha tecnica oficial do U1, em milimetros. E a regua contra a qual a cena e
# conferida: escala errada estraga profundidade de campo, chanfro e luz em
# watts de uma vez so, e nada disso aparece olhando o viewport.
U1_OFICIAL_MM = (584.0, 499.0, 730.0)


def extensao_da_cena(escala):
    # Bounding box no espaco de mundo de tudo que vai aparecer no render. Usa
    # matrix_world porque escala herdada de pai nao entra em obj.dimensions de
    # forma legivel, e e justamente onde a conta costuma furar.
    menor = None
    maior = None
    for obj in bpy.data.objects:
        if obj.type != "MESH" or obj.hide_render:
            continue
        for canto in obj.bound_box:
            p = obj.matrix_world @ Vector(canto)
            if menor is None:
                menor = p.copy()
                maior = p.copy()
            else:
                for eixo in range(3):
                    menor[eixo] = min(menor[eixo], p[eixo])
                    maior[eixo] = max(maior[eixo], p[eixo])
    if menor is None:
        return None
    tamanho = [mm(maior[e] - menor[e], escala) for e in range(3)]
    ordenado = sorted(tamanho, reverse=True)
    esperado = sorted(U1_OFICIAL_MM, reverse=True)
    # Compara os eixos ordenados porque o modelo pode estar deitado ou girado,
    # e nesse caso a proporcao ainda bate mesmo com os eixos trocados.
    # Um fator por eixo, nao so pelo maior: comparar so o maior eixo dizia
    # "0,82x do tamanho real" para uma placa de 600x600x1,5, que nao e o U1 em
    # escala quase certa - e outra coisa. So e escala quando os tres fatores
    # concordam entre si; discordancia significa proporcao diferente, e ai o
    # conselho de multiplicar mandaria escalar errado.
    fatores = [
        round(esperado[e] / ordenado[e], 4) if ordenado[e] > 1e-6 else None
        for e in range(3)
    ]
    validos = [f for f in fatores if f]
    fator = None
    proporcao_bate = False
    if len(validos) == 3:
        proporcao_bate = (max(validos) / min(validos)) <= 1.15
        if proporcao_bate:
            fator = round(sum(validos) / 3.0, 4)
    return {
        "fatores_por_eixo": fatores,
        "proporcao_bate_com_o_u1": proporcao_bate,
        "minimo_mm": [mm(v, escala) for v in menor],
        "maximo_mm": [mm(v, escala) for v in maior],
        "tamanho_mm": tamanho,
        "oficial_mm": list(U1_OFICIAL_MM),
        "fator_para_bater_com_o_oficial": fator,
    }


def analisar_malha(obj, escala):
    malha = obj.data
    total = len(malha.polygons)

    # foreach_get em vez de laco Python: malha de impressora passa facil de um
    # milhao de faces, e o laco travaria o Blender por minutos.
    lados = [0] * total
    if total:
        malha.polygons.foreach_get("loop_total", lados)
    contagem = Counter(lados)
    triangulos = contagem.get(3, 0)
    quadrilateros = contagem.get(4, 0)
    ngons = total - triangulos - quadrilateros

    suaves = [False] * total
    if total:
        malha.polygons.foreach_get("use_smooth", suaves)
    faces_suaves = sum(1 for s in suaves if s)

    ficha = {
        "nome": obj.name,
        "tipo": obj.type,
        "vertices": len(malha.vertices),
        "arestas": len(malha.edges),
        "faces": total,
        "triangulos": triangulos,
        "quadrilateros": quadrilateros,
        "ngons": ngons,
        "faces_com_sombreamento_suave": faces_suaves,
        "mapas_uv": [uv.name for uv in malha.uv_layers],
        "tem_uv": len(malha.uv_layers) > 0,
        "materiais": [m.name if m else "(vazio)" for m in obj.data.materials],
        "modificadores": [(m.name, m.type) for m in obj.modifiers],
        "escala": [round(v, 6) for v in obj.scale],
        "escala_de_mundo": [round(v, 6) for v in obj.matrix_world.to_scale()],
        "escala_aplicada": all(abs(v - 1.0) < 1e-6 for v in obj.scale),
        "dimensoes_mm": [mm(d, escala) for d in obj.dimensions],
        "pai": obj.parent.name if obj.parent else None,
        "colecoes": [c.name for c in obj.users_collection],
        "visivel": not obj.hide_viewport,
        "renderizavel": not obj.hide_render,
    }

    # Normais customizadas e o "Smooth by Angle" mudam completamente como o
    # metal reage a luz. Em 4.1+ o auto smooth virou modificador, entao os dois
    # jeitos sao consultados e a ausencia do atributo nao pode derrubar o
    # script no Blender do outro lado.
    try:
        ficha["normais_customizadas"] = bool(malha.has_custom_normals)
    except AttributeError:
        ficha["normais_customizadas"] = None
    try:
        ficha["auto_smooth_legado"] = bool(malha.use_auto_smooth)
    except AttributeError:
        ficha["auto_smooth_legado"] = None

    return ficha


def analisar_cena():
    cena = bpy.context.scene
    escala = cena.unit_settings.scale_length

    objetos = []
    for obj in bpy.data.objects:
        if obj.type == "MESH":
            objetos.append(analisar_malha(obj, escala))
        else:
            objetos.append(
                {
                    "nome": obj.name,
                    "tipo": obj.type,
                    "dimensoes_mm": [mm(d, escala) for d in obj.dimensions],
                    "pai": obj.parent.name if obj.parent else None,
                    "colecoes": [c.name for c in obj.users_collection],
                    "visivel": not obj.hide_viewport,
                    "renderizavel": not obj.hide_render,
                }
            )

    mundo = cena.world
    textura_de_mundo = []
    if mundo and mundo.use_nodes:
        for no in mundo.node_tree.nodes:
            if no.type == "TEX_ENVIRONMENT":
                imagem = getattr(no, "image", None)
                textura_de_mundo.append(imagem.name if imagem else "(sem imagem)")

    return {
        "extensao": extensao_da_cena(escala),
        "blender": bpy.app.version_string,
        "arquivo": bpy.data.filepath or "(nao salvo)",
        "sistema": sys.platform,
        "cena": {
            "nome": cena.name,
            "motor": cena.render.engine,
            "resolucao": [
                cena.render.resolution_x,
                cena.render.resolution_y,
                cena.render.resolution_percentage,
            ],
            "fps": cena.render.fps / max(cena.render.fps_base, 1e-9),
            "quadros": [cena.frame_start, cena.frame_end],
            "sistema_de_unidades": cena.unit_settings.system,
            "escala_de_unidade": escala,
        },
        "mundo": {
            "nome": mundo.name if mundo else None,
            "texturas_de_ambiente": textura_de_mundo,
        },
        "totais": {
            "objetos": len(bpy.data.objects),
            "malhas": sum(1 for o in bpy.data.objects if o.type == "MESH"),
            "cameras": sum(1 for o in bpy.data.objects if o.type == "CAMERA"),
            "luzes": sum(1 for o in bpy.data.objects if o.type == "LIGHT"),
            "vazios": sum(1 for o in bpy.data.objects if o.type == "EMPTY"),
            "materiais": len(bpy.data.materials),
            "imagens": len(bpy.data.images),
            "colecoes": len(bpy.data.collections),
            "faces": sum(
                len(o.data.polygons) for o in bpy.data.objects if o.type == "MESH"
            ),
        },
        "colecoes": [
            {
                "nome": c.name,
                "objetos": len(c.objects),
                "filhas": [f.name for f in c.children],
            }
            for c in bpy.data.collections
        ],
        "objetos": objetos,
    }


def veredito(dados):
    # A conclusao que importa para o anuncio, escrita por extenso: quem le o
    # relatorio precisa saber o que da e o que nao da para filmar, nao so os
    # numeros crus.
    linhas = []
    malhas = [o for o in dados["objetos"] if o["tipo"] == "MESH"]
    faces = dados["totais"]["faces"]

    if dados["totais"]["malhas"] <= 2:
        linhas.append(
            "MALHA UNICA: o U1 esta em 1 ou 2 objetos. Nao da para animar troca "
            "de cabecote nem isolar peca em close sem separar antes."
        )
    else:
        linhas.append(
            "PECAS SEPARADAS: %d malhas. Da para close por peca e para animar "
            "partes moveis." % dados["totais"]["malhas"]
        )

    so_triangulos = [
        o for o in malhas if o["faces"] and o["triangulos"] == o["faces"]
    ]
    if len(so_triangulos) == len(malhas) and malhas:
        linhas.append(
            "MALHA TRIANGULADA em tudo: cara de STL/CAD exportado. Sem aresta "
            "de apoio, chanfro em close sai serrilhado - remodelagem confirmada."
        )
    elif so_triangulos:
        linhas.append(
            "%d de %d malhas sao 100%% trianguladas." % (len(so_triangulos), len(malhas))
        )

    sem_uv = [o for o in malhas if not o["tem_uv"]]
    if sem_uv:
        linhas.append(
            "%d malhas sem UV: material com textura de imagem nao gruda; ou "
            "desdobra, ou o material vira procedural." % len(sem_uv)
        )

    torta = [o for o in malhas if not o["escala_aplicada"]]
    if torta:
        linhas.append(
            "%d objetos com escala nao aplicada: bevel e deslocamento saem "
            "errados. Ctrl+A > Scale resolve." % len(torta)
        )

    sem_material = [o for o in malhas if not o["materiais"]]
    if sem_material:
        linhas.append(
            "%d malhas sem material nenhum - o visual todo ainda vai ser feito."
            % len(sem_material)
        )


    ext = dados.get("extensao")
    if ext:
        t = ext["tamanho_mm"]
        fator = ext["fator_para_bater_com_o_oficial"]
        linhas.append(
            "Extensao da cena: %.0f x %.0f x %.0f mm (oficial do U1: 584 x 499 x 730)."
            % (t[0], t[1], t[2])
        )
        if not ext["proporcao_bate_com_o_u1"]:
            linhas.append(
                "PROPORCAO DIFERENTE do U1 (fatores por eixo: %s). Ou a cena tem "
                "so parte da maquina, ou tem objeto extra entrando na medida, ou "
                "nao e o U1 inteiro. Nao da para corrigir com um fator so."
                % ext["fatores_por_eixo"]
            )
        elif fator > 1.15 or fator < 0.87:
            linhas.append(
                "ESCALA FORA: o modelo esta %.4gx do tamanho real. Multiplicar "
                "por %s e aplicar (Ctrl+A > Scale) antes de qualquer luz ou "
                "profundidade de campo." % (1.0 / fator, fator)
            )
        else:
            linhas.append("Escala confere com o produto real.")

    linhas.append(
        "Orcamento de poligonos: %s faces no total. EEVEE Next na 4050 aguenta "
        "folgado ate ~5 milhoes." % f"{faces:,}".replace(",", ".")
    )

    if not dados["totais"]["luzes"] and not dados["mundo"]["texturas_de_ambiente"]:
        linhas.append("Sem luz e sem HDRI: iluminacao inteira a construir.")

    return linhas


def main():
    dados = analisar_cena()
    dados["veredito"] = veredito(dados)

    destino = pasta_de_saida()
    caminho_json = os.path.join(destino, "u1_diagnostico.json")
    caminho_txt = os.path.join(destino, "u1_diagnostico.txt")

    with open(caminho_json, "w", encoding="utf-8") as f:
        json.dump(dados, f, indent=2, ensure_ascii=False)

    malhas = [o for o in dados["objetos"] if o["tipo"] == "MESH"]
    malhas.sort(key=lambda o: o["faces"], reverse=True)

    linhas = []
    linhas.append("=" * 72)
    linhas.append("DIAGNOSTICO DA CENA - anuncio Snapmaker U1")
    linhas.append("=" * 72)
    linhas.append("Blender %s  |  %s" % (dados["blender"], dados["sistema"]))
    linhas.append("Arquivo: %s" % dados["arquivo"])
    c = dados["cena"]
    linhas.append(
        "Cena '%s': motor %s, %dx%d @ %d%%, %.3g fps, quadros %d-%d"
        % (
            c["nome"],
            c["motor"],
            c["resolucao"][0],
            c["resolucao"][1],
            c["resolucao"][2],
            c["fps"],
            c["quadros"][0],
            c["quadros"][1],
        )
    )
    linhas.append(
        "Unidades: %s, escala %s" % (c["sistema_de_unidades"], c["escala_de_unidade"])
    )
    t = dados["totais"]
    linhas.append(
        "Totais: %d objetos (%d malhas, %d cameras, %d luzes, %d vazios), "
        "%d materiais, %d imagens, %d colecoes"
        % (
            t["objetos"],
            t["malhas"],
            t["cameras"],
            t["luzes"],
            t["vazios"],
            t["materiais"],
            t["imagens"],
            t["colecoes"],
        )
    )
    linhas.append("Faces somadas: %s" % f"{t['faces']:,}".replace(",", "."))
    if dados["extensao"]:
        e = dados["extensao"]["tamanho_mm"]
        linhas.append("Extensao no mundo: %.1f x %.1f x %.1f mm" % (e[0], e[1], e[2]))
    if dados["mundo"]["texturas_de_ambiente"]:
        linhas.append("HDRI no mundo: %s" % ", ".join(dados["mundo"]["texturas_de_ambiente"]))

    linhas.append("")
    linhas.append("-" * 72)
    linhas.append("VEREDITO")
    linhas.append("-" * 72)
    for v in dados["veredito"]:
        linhas.append("* " + v)

    linhas.append("")
    linhas.append("-" * 72)
    linhas.append("COLECOES")
    linhas.append("-" * 72)
    for col in dados["colecoes"]:
        filhas = (" > " + ", ".join(col["filhas"])) if col["filhas"] else ""
        linhas.append("  %-34s %4d objetos%s" % (col["nome"], col["objetos"], filhas))

    linhas.append("")
    linhas.append("-" * 72)
    linhas.append(
        "MALHAS (%d maiores de %d, por numero de faces)"
        % (min(MAIORES_NO_TEXTO, len(malhas)), len(malhas))
    )
    linhas.append("-" * 72)
    linhas.append(
        "%-30s %9s %6s %6s %6s %4s %s"
        % ("nome", "faces", "tri%", "quad%", "ngon", "uv", "dimensoes mm")
    )
    for o in malhas[:MAIORES_NO_TEXTO]:
        f_total = max(o["faces"], 1)
        d = o["dimensoes_mm"]
        linhas.append(
            "%-30s %9s %5.0f%% %5.0f%% %6d %4s %gx%gx%g"
            % (
                o["nome"][:30],
                f"{o['faces']:,}".replace(",", "."),
                100.0 * o["triangulos"] / f_total,
                100.0 * o["quadrilateros"] / f_total,
                o["ngons"],
                "sim" if o["tem_uv"] else "NAO",
                round(d[0], 1),
                round(d[1], 1),
                round(d[2], 1),
            )
        )
        if o["materiais"]:
            linhas.append("%-30s   materiais: %s" % ("", ", ".join(o["materiais"])))
        if o["modificadores"]:
            linhas.append(
                "%-30s   modificadores: %s"
                % ("", ", ".join("%s(%s)" % (n, t) for n, t in o["modificadores"]))
            )

    nao_malhas = [o for o in dados["objetos"] if o["tipo"] != "MESH"]
    if nao_malhas:
        linhas.append("")
        linhas.append("-" * 72)
        linhas.append("OUTROS OBJETOS")
        linhas.append("-" * 72)
        for o in nao_malhas:
            linhas.append("  %-30s %s" % (o["nome"][:30], o["tipo"]))

    texto = "\n".join(linhas)
    with open(caminho_txt, "w", encoding="utf-8") as f:
        f.write(texto + "\n")

    print("\n" + texto + "\n")
    print("=" * 72)
    print("RELATORIO SALVO EM:")
    print("  " + caminho_txt)
    print("  " + caminho_json)
    print("Manda o .txt (ou o conteudo dele) para eu construir o anuncio.")
    print("=" * 72)


main()
