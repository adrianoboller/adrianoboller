/* prova.c -- o PhxSql embutido exercitado de C, que e o ponto da frente.
 *
 *     bancada/embutido/provar.sh
 *
 * Compila e RODA em x86-64 e, sob qemu-aarch64-static, em ARM64. A distincao
 * importa: esta casa ja escreveu que "compila" nao e "roda", e foi um `.so`
 * ARM64 executado sob emulacao que transformou uma promessa em prova.
 *
 * O diretorio de dados vem em argv[1]; o programa nao cria nem apaga nada
 * fora dele.
 *
 * # Por que este arquivo nao inclui <string.h> nem <stdlib.h>
 *
 * Porque ele e compilado para DUAS arquiteturas nesta maquina, e aqui nao ha
 * sysroot de aarch64 -- os cabecalhos do sistema sao do x86-64. Com
 * -nostdlibinc o clang usa so os cabecalhos DELE (stdint.h, stddef.h), que
 * sao por-alvo e corretos nos dois casos, e as tres funcoes de libc que este
 * programa usa entram declaradas a mao. E C valido, e mantem o MESMO fonte
 * nos dois lados -- que e o que faz a prova valer.
 */

#include "phxsql.h"

#ifdef PHX_SEM_CABECALHOS
extern int printf(const char *fmt, ...);
#else
#include <stdio.h>
#endif

/* ------------------------------------------------------------- aferidor */

static int erros = 0;
static int passos = 0;

static void conferir(int condicao, const char *o_que) {
    passos++;
    if (condicao) {
        printf("  ok    %s\n", o_que);
    } else {
        printf("  FALHA %s\n", o_que);
        erros++;
    }
}

static void mostrar_erro(const char *onde, int32_t codigo) {
    uint8_t msg[512];
    size_t precisa = 0;
    phx_ultimo_erro(msg, sizeof msg, &precisa);
    if (precisa >= sizeof msg) precisa = sizeof msg - 1;
    msg[precisa] = 0;
    printf("        %s: %d (%s) %s\n", onde, codigo, phx_erro_nome(codigo),
           (const char *)msg);
}

static size_t tamanho(const char *s) {
    size_t n = 0;
    while (s[n]) n++;
    return n;
}

static int iguais(const uint8_t *a, const uint8_t *b, size_t n) {
    for (size_t i = 0; i < n; i++) if (a[i] != b[i]) return 0;
    return 1;
}

/* --------------------------------------------------------------- montar */

static int montar(PhxBase *base, const char *nome, PhxTabela **saida) {
    PhxEsquema *e = 0;
    int32_t r = phx_esquema_novo((const uint8_t *)nome, tamanho(nome), &e);
    if (r != PHX_OK) { mostrar_erro("esquema_novo", r); return 0; }

    r  = phx_esquema_coluna(e, PHX_T("id"),     PHX_COL_INT8, 0, 0, 0,
                            PHX_COL_OBRIGATORIA);
    r |= phx_esquema_coluna(e, PHX_T("nome"),   PHX_COL_STR, 60, 0, 0,
                            PHX_COL_OBRIGATORIA);
    r |= phx_esquema_coluna(e, PHX_T("cidade"), PHX_COL_STR, 40, 0, 0, 0);
    r |= phx_esquema_coluna(e, PHX_T("ficha"),  PHX_COL_MEMO, 0, 0, 0, 0);
    r |= phx_esquema_indice(e, PHX_T("porId"), PHX_IDX_UNICO | PHX_IDX_PRIMARIA);
    r |= phx_esquema_indice_coluna(e, 0, 0);
    r |= phx_esquema_indice(e, PHX_T("porNome"), 0);
    r |= phx_esquema_indice_coluna(e, 1, PHX_IDX_SEM_CAIXA);
    if (r != PHX_OK) { mostrar_erro("montar esquema", r); phx_esquema_liberar(e); return 0; }

    r = phx_tabela_criar(base, 0, 0, e, saida);
    if (r != PHX_OK) mostrar_erro("tabela_criar", r);
    phx_esquema_liberar(e);
    return r == PHX_OK;
}

static uint64_t inserir(PhxTabela *t, int64_t id, const char *nome,
                        const char *cidade) {
    PhxValor linha[4];
    linha[0] = phx_int(id);
    linha[1] = phx_texto(nome, tamanho(nome));
    linha[2] = phx_texto(cidade, tamanho(cidade));
    linha[3] = phx_nulo();
    uint64_t rowid = 0;
    int32_t r = phx_inserir(t, linha, 4, &rowid);
    if (r != PHX_OK) { mostrar_erro("inserir", r); return 0; }
    return rowid;
}

/* ------------------------------------------------------------------ main */

int main(int argc, char **argv) {
    if (argc < 2) {
        printf("uso: prova <diretorio-de-dados>\n");
        return 2;
    }
    const char *raiz = argv[1];

    uint8_t versao[64];
    size_t precisa = 0;
    phx_versao(versao, sizeof versao, &precisa);
    versao[precisa] = 0;
    printf("PhxSql embutido %s -- ABI de C\n\n", (const char *)versao);

    /* ------------------------------------------------------ 1. o ciclo */
    printf("1. abrir, criar, gravar e ler\n");
    PhxBase *base = 0;
    int32_t r = phx_base_abrir((const uint8_t *)raiz, tamanho(raiz),
                               PHX_T("vendas"), PHX_CRIAR, &base);
    conferir(r == PHX_OK, "phx_base_abrir com PHX_CRIAR");
    if (r != PHX_OK) { mostrar_erro("base_abrir", r); return 1; }

    PhxTabela *t = 0;
    conferir(montar(base, "clientes", &t), "phx_tabela_criar");
    if (!t) return 1;

    uint64_t r1 = inserir(t, 1, "Adriano Boller", "Blumenau");
    uint64_t r2 = inserir(t, 2, "Marcia Alves", "Joinville");
    conferir(r1 == 1 && r2 == 2, "o rowid e a ordem de digitacao");

    uint64_t qtd = 0;
    phx_tabela_registros(t, PHX_VISAO_ATIVAS, &qtd);
    conferir(qtd == 2, "phx_tabela_registros ve as duas");

    PhxLinha *l = 0;
    r = phx_ler(t, r1, &l);
    conferir(r == PHX_OK, "phx_ler do rowid 1");
    if (r == PHX_OK) {
        const PhxValor *v = 0;
        size_t n = 0;
        phx_linha_valores(l, &v, &n);
        conferir(v[0].tipo == PHX_INT && v[0].numero == 1, "coluna id");
        conferir(v[1].tipo == PHX_TEXTO && v[1].tam == 14 &&
                 iguais(v[1].dados, (const uint8_t *)"Adriano Boller", 14),
                 "coluna nome, com tamanho explicito");
        printf("        linha 1: id=%lld nome=%.*s cidade=%.*s\n",
               (long long)v[0].numero,
               (int)v[1].tam, (const char *)v[1].dados,
               (int)v[2].tam, (const char *)v[2].dados);
        phx_linha_liberar(l);
    }

    /* ------------------------------------------------ 2. cursor e busca */
    printf("\n2. varrer com cursor e buscar por indice\n");
    PhxCursor *c = 0;
    r = phx_cursor_abrir(t, PHX_VISAO_ATIVAS, &c);
    conferir(r == PHX_OK, "phx_cursor_abrir");
    uint64_t vistos = 0, id = 0;
    while (phx_cursor_proximo(t, c, &id) == PHX_OK) vistos++;
    conferir(vistos == 2, "o cursor entregou as duas linhas");
    phx_cursor_liberar(c);

    PhxValor chave = phx_int(2);
    uint64_t achados_ids[4];
    size_t achados = 0;
    r = phx_buscar(t, PHX_T("porId"), &chave, 1, achados_ids, 4, &achados);
    conferir(r == PHX_OK && achados == 1 && achados_ids[0] == r2,
             "phx_buscar pela chave primaria");

    /* Indice NOCASE: o mesmo nome em caixa diferente tem de achar. */
    PhxValor pnome = phx_texto("adriano boller", 14);
    r = phx_buscar(t, PHX_T("porNome"), &pnome, 1, achados_ids, 4, &achados);
    conferir(r == PHX_OK && achados == 1 && achados_ids[0] == r1,
             "phx_buscar num indice sem distinguir caixa");

    /* --------------------------------------------------- 3. o byte zero */
    printf("\n3. o byte zero no dado do cliente\n");
    static const uint8_t ficha[] = { 'a','n','t','e','s',0,'d','e','p','o','i','s' };
    PhxValor linha[4];
    linha[0] = phx_int(3);
    linha[1] = phx_texto("Com Zero", 8);
    linha[2] = phx_nulo();
    linha[3] = phx_memo(ficha, sizeof ficha);
    uint64_t r3 = 0;
    r = phx_inserir(t, linha, 4, &r3);
    conferir(r == PHX_OK, "gravar um memo com \\0 no meio");
    if (r == PHX_OK) {
        r = phx_ler(t, r3, &l);
        const PhxValor *v = 0;
        size_t n = 0;
        phx_linha_valores(l, &v, &n);
        conferir(v[3].tam == sizeof ficha &&
                 iguais(v[3].dados, ficha, sizeof ficha),
                 "voltou inteiro -- nada de strlen na fronteira");
        phx_linha_liberar(l);
    }

    /* ------------------------------------------- 4. os caminhos de erro */
    printf("\n4. os caminhos de erro\n");
    linha[0] = phx_int(1);   /* chave que ja existe */
    linha[1] = phx_texto("Outro", 5);
    linha[2] = phx_nulo();
    linha[3] = phx_nulo();
    uint64_t ignorado = 0;
    r = phx_inserir(t, linha, 4, &ignorado);
    conferir(r == PHX_DUPLICADO, "chave duplicada devolve 3002");
    mostrar_erro("duplicada", r);

    static const uint8_t torto[] = { 0xff, 0xfe, 0xfd };
    linha[0] = phx_int(9);
    linha[1] = phx_texto(torto, sizeof torto);
    r = phx_inserir(t, linha, 4, &ignorado);
    conferir(r == PHX_ERRO_UTF8, "texto que nao e UTF-8 e recusado");

    size_t falta = 0;
    r = phx_versao(0, 0, &falta);
    conferir(r == PHX_ERRO_BUFFER && falta > 0,
             "buffer pequeno diz quanto falta, e nao trunca calado");

    uint64_t nada = 0;
    r = phx_tabela_registros(0, PHX_VISAO_ATIVAS, &nada);
    conferir(r == PHX_ERRO_PONTEIRO, "punho nulo nao derruba nada");

    r = phx_ler(t, 9999, &l);
    conferir(r == PHX_NAO_HA, "linha que nao existe e PHX_NAO_HA, nao erro");

    /* ------------------------------------------- 5. janela de conflito */
    printf("\n5. a janela de conflito (guarda pedida, nao imposta)\n");
    uint64_t versao_lida = 0;
    phx_versao_da_linha(t, r1, &versao_lida);
    linha[0] = phx_int(1);
    linha[1] = phx_texto("Adriano B.", 10);
    linha[2] = phx_texto("Blumenau", 8);
    linha[3] = phx_nulo();
    r = phx_atualizar(t, r1, linha, 4);           /* outra sessao gravou */
    conferir(r == PHX_OK, "phx_atualizar sem versao grava como sempre");
    linha[1] = phx_texto("Adriano C.", 10);
    r = phx_atualizar_se(t, r1, linha, 4, versao_lida);
    conferir(r == PHX_CONFLITO, "phx_atualizar_se recusa a versao velha");

    /* ------------------------------------------------- 6. o panico */
    printf("\n6. o panico que nao atravessa\n");
    printf("        (a mensagem do panico sai no stderr -- e esperada)\n");
    r = phx_inserir(t, linha, (size_t)-1, &ignorado);
    conferir(r == PHX_ERRO_PANICO, "contagem absurda vira erro, e nao aborta");
    r = phx_tabela_registros(t, PHX_VISAO_ATIVAS, &nada);
    conferir(r == PHX_ERRO_ENVENENADO, "o punho fica envenenado depois");
    conferir(phx_tabela_fechar(t) == PHX_OK, "e so o fechar continua passando");
    t = 0;

    /* Reabrir e o conserto -- e ele funciona. */
    r = phx_tabela_abrir(base, PHX_T("clientes"), &t);
    conferir(r == PHX_OK, "reabrir e o conserto");
    phx_tabela_registros(t, PHX_VISAO_ATIVAS, &qtd);
    conferir(qtd == 3, "e as tres linhas continuam la");

    /* ---------------------------------------------- 7. punho liberado */
    printf("\n7. a etiqueta do punho\n");
    PhxTabela *morto = 0;
    phx_tabela_abrir(base, PHX_T("clientes"), &morto);
    phx_tabela_fechar(morto);
    r = phx_tabela_registros(morto, PHX_VISAO_ATIVAS, &nada);
    conferir(r == PHX_ERRO_PONTEIRO, "punho ja liberado e recusado");
    conferir(phx_tabela_fechar(morto) == PHX_ERRO_PONTEIRO,
             "liberar duas vezes e recusado");

    /* ------------------------------------------------- 8. replicacao */
    printf("\n8. replicacao pelos ganchos\n");
    PhxBase *espelho = 0;
    r = phx_base_abrir((const uint8_t *)raiz, tamanho(raiz),
                       PHX_T("espelho"), PHX_CRIAR, &espelho);
    conferir(r == PHX_OK, "abrir o database do espelho");
    PhxTabela *b = 0;
    conferir(montar(espelho, "clientes", &b), "criar a tabela do espelho");

    PhxTabela *fonte = 0;
    PhxEsquema *nada_esq = 0;
    (void)nada_esq;
    PhxBase *base2 = 0;
    r = phx_base_abrir((const uint8_t *)raiz, tamanho(raiz),
                       PHX_T("origem"), PHX_CRIAR, &base2);
    conferir(r == PHX_OK, "abrir o database de origem");
    conferir(montar(base2, "clientes", &fonte), "criar a tabela de origem");

    phx_imagem_no_diario(fonte, 1);
    phx_forcar_proximo_evento(fonte, 0, 7);   /* origem 7 */
    inserir(fonte, 10, "Replicado Um", "Blumenau");
    inserir(fonte, 20, "Replicado Dois", "Itajai");

    uint64_t eventos = 0;
    phx_diario_qtd(fonte, &eventos);
    conferir(eventos == 2, "o diario tem dois eventos");

    PhxEvento lista[8];
    size_t lidos = 0;
    r = phx_diario_ler(fonte, 0, lista, 8, &lidos);
    conferir(r == PHX_OK && lidos == 2, "phx_diario_ler");
    conferir(lista[0].operacao == PHX_OP_INCLUSAO && lista[0].origem == 7,
             "a operacao e a origem atravessam");

    int aplicados = 0;
    for (uint64_t i = 0; i < eventos; i++) {
        PhxEvento ev;
        PhxImagem *img = 0;
        if (phx_diario_evento_com_imagem(fonte, i, &ev, &img) != PHX_OK) break;
        const uint8_t *dados = 0;
        size_t tam = 0;
        if (phx_imagem_bytes(img, &dados, &tam) != PHX_OK) break;
        uint64_t saiu = 0;
        int32_t ra = phx_aplicar_evento(b, ev.operacao, ev.rowid, dados, tam, &saiu);
        if (ra != PHX_OK) { mostrar_erro("aplicar_evento", ra); break; }
        if (saiu == ev.rowid) aplicados++;
        phx_imagem_liberar(img);
    }
    conferir(aplicados == 2, "o espelho aplicou os dois com o MESMO rowid");
    phx_tabela_registros(b, PHX_VISAO_ATIVAS, &qtd);
    conferir(qtd == 2, "e as duas linhas estao la");

    r = phx_ler(b, 2, &l);
    if (r == PHX_OK) {
        const PhxValor *v = 0;
        size_t n = 0;
        phx_linha_valores(l, &v, &n);
        conferir(v[1].tam == 14 &&
                 iguais(v[1].dados, (const uint8_t *)"Replicado Dois", 14),
                 "e o conteudo chegou inteiro");
        printf("        do espelho: %.*s / %.*s\n",
               (int)v[1].tam, (const char *)v[1].dados,
               (int)v[2].tam, (const char *)v[2].dados);
        phx_linha_liberar(l);
    }

    /* --------------------------------------------------- 9. integridade */
    printf("\n9. integridade e descarga\n");
    PhxRelatorio rel;
    conferir(phx_sincronizar(t) == PHX_OK, "phx_sincronizar");
    r = phx_verificar(t, &rel);
    conferir(r == PHX_OK && rel.registros == 3, "phx_verificar");
    printf("        registros=%llu slots=%llu indices=%llu eventos=%llu\n",
           (unsigned long long)rel.registros, (unsigned long long)rel.slots,
           (unsigned long long)rel.indices, (unsigned long long)rel.eventos);

    size_t tabelas = 0;
    phx_base_tabelas_qtd(base, &tabelas);
    conferir(tabelas == 1, "phx_base_tabelas_qtd");
    uint8_t nome_tab[64];
    size_t pn = 0;
    phx_base_tabela_nome(base, 0, nome_tab, sizeof nome_tab, &pn);
    nome_tab[pn] = 0;
    printf("        tabela: %s\n", (const char *)nome_tab);

    phx_tabela_fechar(t);
    phx_tabela_fechar(b);
    phx_tabela_fechar(fonte);
    phx_base_fechar(base);
    phx_base_fechar(base2);
    phx_base_fechar(espelho);

    printf("\n%d passos, %d falhas\n", passos, erros);
    return erros == 0 ? 0 : 1;
}
