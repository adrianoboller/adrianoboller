#!/usr/bin/env python3
"""Gera a pagina do organograma de arquivos do projeto de destino.

Existe como script, e nao como HTML escrito a mao, pela regra do projeto:
numero visivel sai de um gerador. As contagens (arquivos que o questionario
pode gerar, versao) vem de docs/dossie/numeros.json, medido no repositorio.

Uso: python3 docs/dossie/gerar-organograma.py [saida.html]
"""
from __future__ import annotations

import html as H
import json
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
E = H.escape

# (caminho, papel, quem cria: q=questionario, p=PMO, u=usuario, g=gates/hooks)
CAMADAS = [
 ("Raiz · o que a sessão lê antes de qualquer comando", "A primeira sessão começa aqui, na ordem em que aparecem.", [
  ("CLAUDE.md", "regras do projeto, respostas, corpus 12k, identificação, skills de ERP, artefatos", "q"),
  ("INDEX_FILES.md", "mapa: uma linha por arquivo, o que é e quando abrir; regravado sempre", "q"),
  ("AGENTS.md", "como um agente trabalha neste ERP: ordem de leitura, regras, módulo → skill", "q"),
  ("CONTEXT.md · CONTEXT-MAP.md", "finalidade e módulos (bloco 0); quem é dono de qual regra", "q"),
  ("UBIQUITOUS_LANGUAGE.md", "um termo, um significado, um nome no código; nome do legado ao lado", "q"),
  ("ARCHITECTURE.md · SECURITY.md", "monólito modular, outbox, auditoria; papéis, segredos, LGPD", "q"),
  ("DESIGN.md · PRODUCT.md", "tela modelo, botões, cores (letra F); quem opera e em que condições", "q"),
  ("CHANGELOG.md · .editorconfig · .gitignore", "histórico, estilo de código, o que nunca se versiona", "q"),
 ]),
 (".claude/ · o Claude Code do projeto", "Hooks e skills próprias do projeto; o plugin fica fora, instalado uma vez.", [
  ("settings.json", "permissões (nega ler .env) e hooks: testar ao parar, lint ao editar", "q"),
  ("hooks/testar.sh · hooks/lint.sh", "os comandos de L4, rodados pelo Claude Code", "q"),
  ("skills/regras-do-legado/SKILL.md", "origem localizável, matriz, golden master; hierarquia de evidências", "q"),
  ("skills/legado-para-destino/SKILL.md", "o que cada peça do WX vira no destino e a estratégia escolhida", "q"),
 ]),
 ("inputs/ · evidências do legado (somente leitura)", "Anexo só é evidência depois de lido. Um hook recusa qualquer escrita aqui — mas ler e redirecionar para fora é liberado.", [
  ("banco.sql", "script do banco HFSQL exportado (letra A)", "u"),
  ("*.pdf", "código, interfaces, queries e o completo (B a E), lidos por localizador página#linha", "u"),
  ("screenshots/ · screenshots.json", "capturas das telas; a tela modelo (F0) entre elas", "u"),
  ("dados-de-amostra/ · marca/", "dados anonimizados para o golden master; logotipos da empresa e do software", "u"),
  ("<código PHP>", "quando o legado tem PHP (projeto.legado_php); a skill php-legado-e-destino guia a leitura", "u"),
 ]),
 ("artefatos/ · o que o cliente mandou por fora (bloco M)", "Somente leitura como inputs/, e por um motivo a mais: o catálogo guarda o hash de cada arquivo, e editá-lo à mão faria o catálogo mentir.", [
  ("CATALOGO.md", "o que foi submetido, por tipo, com onde usar, data e SHA-256; regravado pelo script", "q"),
  ("registro.json", "os mesmos fatos em JSON, para outro script ler", "q"),
  ("LEIA-ME.md", "como submeter um artefato e por que a pasta não se edita", "q"),
  ("anotacao/ · classe-oop/ · query-sql/ · relatorio/ · codigo-php/ …", "uma pasta por tipo; onze tipos aceitos, cada arquivo com onde_usar obrigatório", "u"),
 ]),
 (".wx-migration/ · governança da conversão", "Tudo que o plugin sabe sobre este projeto. É o que a exportação e o PMO carregam.", [
  ("questionario.json → respostas_questionario.md", "as 60 respostas, cruas e legíveis; o markdown abre com índice por id e o estado de cada uma, e é onde um agente procura antes de perguntar", "q"),
  ("wx-inputs.manifest.json · conversion.config.json", "o que o G0 lê e como (modo, destino, fidelidade)", "q"),
  ("traceability.csv · gaps.md", "a matriz BR-/QRY-/UI-/INT-/RPT-/DB- com estado, e as lacunas GAP-*", "q"),
  ("empresa.md · entrega.json", "softhouse, diretores, pessoal; GitHub, branch, nome da credencial, diretório", "q"),
  ("processo-de-conversao.md", "o que cada peça vira, gate a gate, e a estratégia (H/I)", "q"),
  ("prompts/kickoff.md · prompts/prototipacao.md", "a primeira sessão e a ferramenta de telas (L1, L2)", "q"),
  ("ambiente.md · ambiente/instalar-ambiente.sh · ambiente/.env.exemplo", "ferramentas de K, instalador idempotente, só nomes de variáveis", "q"),
  ("ambiente/backup-e-replicacao.md", "o plano do K8: RPO, RTO, retenção, réplicas, e quando a restauração foi testada de verdade", "q"),
  ("ambiente/papeis-*.sql · ambiente/n8n/", "papéis do banco; compose, banco e integração do n8n (K7)", "q"),
  ("pmo/projeto.json · cronograma · organograma · fluxograma · riscos", "sementes do PMO vindas do bloco 0", "q"),
  ("pmo/backlog.md · kanban · relatorio.md · painel.html · base_de_conhecimento.md", "nascem ao rodar o PMO: sprints, PDCA, relatório de onze seções", "p"),
  ("blocos/ e sprints/ (BlocoNNNN-SPNNNNN-*.md + .zip)", "cada sprint fechada em .md zipado", "p"),
  ("extraidos/<pdf>.md", "PDF virado markdown citável: uma seção por página, hash no cabeçalho, OCR marcado em vez de inventado", "g"),
  ("logs/plugin-AAAA-MM-DD.jsonl", "toda operação do plugin: instante, argumentos, código de saída, duração; as negativas dos hooks entram aqui", "g"),
  ("preflight/runs/ · rag/", "saída do G0 e índice BM25 do RAG; o zelador limpa os velhos", "g"),
 ]),
 ("docs/ · o que se decide e se prova", "Esqueleto de ERP (L6). Cada módulo do bloco 0 tem um domínio próprio.", [
  ("PRD.md · ROADMAP.md · BACKLOG.md", "requisitos da v1 e fora dela; ondas por módulo; épicos", "q"),
  ("adr/0001…0004", "monólito modular, multiempresa, auditoria e outbox, fiscal; «não» também vira ADR", "q"),
  ("domain/<módulo>.md · domain/invariants.md · domain/workflows.md", "entidades, invariantes BR-*, eventos, origem no legado, e a skill erp-* que orienta", "q"),
  ("data/modelo-de-dados.md · erd.md · data-dictionary.md", "regras de tabela (dinheiro, data, constraint, índice), diagramas e dicionário", "q"),
  ("api/openapi.yaml · api/events.asyncapi.yaml", "contratos REST e de eventos, com idempotência", "q"),
  ("security/threat-model.md · requisitos.md · dados-pessoais.md · papeis-e-permissoes.md", "STRIDE por módulo, SEC-*, inventário LGPD, matriz de permissões", "q"),
  ("operations/runbook.md · runbooks/backup-restore.md · incident-response.md", "implantação, backup testado, resposta a incidente", "q"),
  ("testing/estrategia.md", "as sete camadas de teste e o que cada uma prova", "q"),
  ("skills-recomendadas.md", "as skills do catálogo skills.sh que cabem nas respostas; o plugin não instala", "q"),
 ]),
 ("database/ · src/ · tests/", "Código e dados por módulo; nada lê tabela de outro módulo.", [
  ("database/schema · migrations · rollback · seeds · views · procedures", "estado desejado, caminho até ele com rollback pareado, dado anonimizado", "q"),
  ("src/<módulo>/", "um diretório por módulo, fronteiras do CONTEXT-MAP", "q"),
  ("tests/unit · domain · integration · contracts · security · migration · e2e", "uma pasta por camada; teste novo falha com o defeito reposto", "q"),
 ]),
 ("scripts/ · .github/ · implantação", "O que roda fora da sessão.", [
  ("scripts/build · deploy · migration · backup · verification", "idempotentes; nunca leem segredo de arquivo versionado", "q"),
  (".github/workflows/build · tests · security · release.yml", "lint e build; os testes de L4; varredura de segredos; changelog na tag", "q"),
  ("Dockerfile · docker-compose.yml · .mcp.json", "imagem por perfil (L3), serviço + banco, MCPs sem chaves (L5)", "q"),
 ]),
]
LEG = {"q": ("questionário", "gera ao aplicar as respostas"), "p": ("PMO", "nasce ao rodar sprints"),
       "u": ("usuário", "anexo ou artefato, somente leitura"), "g": ("gates e hooks", "saída de execução, temporária")}


def main() -> int:
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "docs/dossie/organograma-de-arquivos.html"
    n = json.loads((RAIZ / "docs/dossie/numeros.json").read_text(encoding="utf-8"))
    linhas = sum(len(c[2]) for c in CAMADAS)
    blocos = "".join(
        f'<section class="camada"><h2>{E(t)}</h2><p>{E(sub)}</p><ul>'
        + "".join(f'<li class="{c}"><code>{E(a)}</code><span>{E(b)}</span></li>' for a, b, c in itens)
        + "</ul></section>" for t, sub, itens in CAMADAS)
    leg = "".join(f'<span class="{k}"><i></i><b>{v[0]}</b> {v[1]}</span>' for k, v in LEG.items())
    page = f'''<title>Organograma de arquivos do projeto</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400;600&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A;--a2:#1F5FBF;--ok:#1F7A4D;--u:#8E5AC8;--grid:#ECEAE3}}
@media (prefers-color-scheme: dark){{:root:not([data-theme="light"]){{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#6FA3FF;--ok:#2FBF71;--u:#B48CF0;--grid:#1B1F33}}}}
:root[data-theme="dark"]{{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#6FA3FF;--ok:#2FBF71;--u:#B48CF0;--grid:#1B1F33}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:15.5px;line-height:1.5}}
.wrap{{max-width:1120px;margin:0 auto;padding:40px 28px 72px}}
h1,h2{{font-family:"Exo 2","Segoe UI",sans-serif;text-wrap:balance;margin:0}}
h1{{font-size:38px;font-weight:800;letter-spacing:-.5px;line-height:1.1;margin-top:8px}}
.eyebrow{{font-family:"Exo 2",sans-serif;font-size:12px;letter-spacing:.14em;text-transform:uppercase;color:var(--a);font-weight:700}}
.lead{{max-width:68ch;color:var(--m);margin:12px 0 0;font-size:16.5px}}
header{{border-bottom:2px solid var(--i);padding-bottom:20px}}
.legenda{{display:flex;flex-wrap:wrap;gap:8px 22px;margin-top:16px;font-family:"Exo 2",sans-serif;font-size:13px;color:var(--m)}}
.legenda i{{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:7px;vertical-align:-1px;background:var(--a)}}
.legenda .p i{{background:var(--a2)}}.legenda .u i{{background:var(--u)}}.legenda .g i{{background:var(--m)}}
.legenda b{{color:var(--i);margin-right:4px}}
.raiz{{margin-top:28px;font-family:"JetBrains Mono",monospace;font-size:14px;color:var(--m)}}
.raiz b{{color:var(--i);font-weight:600}}
.camada{{position:relative;margin:26px 0 0 18px;padding-left:28px;border-left:2px solid var(--l)}}
.camada::before{{content:"";position:absolute;left:-2px;top:14px;width:22px;height:2px;background:var(--l)}}
.camada h2{{font-size:19px;font-weight:700;background:var(--g);display:inline-block;padding-right:8px}}
.camada>p{{margin:2px 0 8px;color:var(--m);font-size:14px;max-width:72ch}}
.camada ul{{list-style:none;margin:0;padding:0;display:grid;gap:6px}}
.camada li{{display:grid;grid-template-columns:minmax(220px,42%) 1fr;gap:14px;align-items:baseline;padding:8px 12px;background:var(--p);border:1px solid var(--l);border-left:3px solid var(--a)}}
.camada li.p{{border-left-color:var(--a2)}}.camada li.u{{border-left-color:var(--u)}}.camada li.g{{border-left-color:var(--m)}}
.camada li code{{font-family:"JetBrains Mono",monospace;font-size:13px;color:var(--i);overflow-wrap:anywhere}}
.camada li span{{color:var(--m);font-size:14px}}
@media(max-width:640px){{.camada li{{grid-template-columns:1fr;gap:2px}}.camada{{margin-left:6px;padding-left:16px}}}}
.nota{{margin-top:32px;color:var(--m);font-size:14px;max-width:74ch;border-top:1px solid var(--l);padding-top:14px}}
.nota code{{font-family:"JetBrains Mono",monospace;font-size:13px}}
</style>
<div class="wrap">
<header>
 <div class="eyebrow">WX Claude Code {E(n["versao"])} · projeto de destino · medido em {E(n["medido_em"])}</div>
 <h1>Organograma de arquivos do projeto</h1>
 <p class="lead">A árvore que o questionário monta no projeto convertido: até {n["arquivos_gerados_pelo_questionario"]} arquivos, aqui agrupados em {linhas} linhas por camada, do jeito que saem para o exemplo ESTOQUE (quatro módulos, esqueleto de ERP ligado, artefatos submetidos). A cor da borda diz quem cria cada um.</p>
 <div class="legenda">{leg}</div>
</header>
<div class="raiz"><b>&lt;projeto&gt;/</b> · exportado para a pasta do usuário como &lt;nome&gt;-&lt;data&gt;</div>
{blocos}
<p class="nota">Ordem de leitura de uma sessão nova: <code>CLAUDE.md</code>, depois <code>INDEX_FILES.md</code> para achar o arquivo certo, depois <code>.wx-migration/respostas_questionario.md</code> antes de perguntar qualquer coisa, e <code>.wx-migration/prompts/kickoff.md</code> para começar. Num módulo, o <code>docs/domain/&lt;módulo&gt;.md</code> diz qual skill <code>erp-*</code> carregar. O que já existe nunca é sobrescrito ao reaplicar o questionário; só <code>INDEX_FILES.md</code> e <code>respostas_questionario.md</code> são regravados, porque são renderização do JSON. Duas pastas são somente leitura por hook: <code>inputs/</code> e <code>artefatos/</code>.</p>
</div>
'''
    saida.write_text(page, encoding="utf-8")
    print(f"ok {saida} ({linhas} linhas, {len(CAMADAS)} camadas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
