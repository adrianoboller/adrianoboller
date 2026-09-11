/* bancada/registro/bench.c
 *
 * Micro-bancada de LATENCIA do Registro do Windows -- ponto por chave.
 *
 * Zero dependencia externa: linka so advapi32 e kernel32, que sao DLLs do
 * proprio Windows -- do mesmo jeito que o resto da casa nao puxa crate. Nao
 * ha biblioteca de terceiro aqui.
 *
 * NAO roda no Linux desta maquina. Compila-se cruzado com
 * x86_64-w64-mingw32-gcc (ver compilar.sh) e roda-se num Windows DE VERDADE:
 * o wine reimplementa o Registro, entao rodar sob wine mediria o wine, e nao
 * o Configuration Manager do kernel do Windows. "O que depende do sistema
 * operacional se prova contra o sistema operacional."
 *
 * Mede a latencia (us/op) de tres coisas, no MESMO valor pequeno, para ser o
 * "trabalho igual" que o Registro consegue fazer contra um banco:
 *   - escrita preguicosa: RegSetValueEx e volta (durabilidade ADIADA -- o
 *     padrao do Registro; a colmeia so vai ao disco na descarga preguicosa
 *     do CM, a cada segundos, ou no RegFlushKey)
 *   - escrita duravel:    RegSetValueEx + RegFlushKey a CADA chave, que
 *     reescreve a colmeia inteira -- e o unico jeito de comparar de igual
 *     para igual com um banco que da fsync por commit
 *   - leitura:            RegQueryValueEx (colmeia quente, em RAM)
 *
 * Os BYTES que de fato vao ao disco NAO saem daqui: um programa de espaco de
 * usuario nao os enxerga sem ETW. Eles saem do Process Monitor (filtro
 * RegSetValue/WriteFile na colmeia) ou de contadores ETW rodando ao lado --
 * ver LEIA-ME.md. Este programa mede o RELOGIO por operacao; o disco e a
 * outra metade, e as duas juntas sao a resposta.
 *
 * Prova real nos dois sentidos: grava um padrao que depende do indice e le
 * de volta conferindo cada chave. Se um valor nao bater, imprime FALHOU e
 * sai != 0 -- nao publica numero de uma corrida que nao provou o que gravou.
 */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SUBCHAVE "Software\\PhxSqlBench"

static double us(LONGLONG dt, LONGLONG freq) {
    return (double)dt * 1e6 / (double)freq;
}

static void nome(char *b, size_t n, int i) {
    snprintf(b, n, "v%08d", i);
}

/* Padrao verificavel: cada byte depende do indice da chave e da posicao,
   entao trocar duas chaves ou truncar um valor e' pego pela conferencia. */
static void encher(unsigned char *buf, int tam, int idx) {
    int j;
    for (j = 0; j < tam; j++) {
        buf[j] = (unsigned char)((idx * 131 + j) & 0xFF);
    }
}

int main(int argc, char **argv) {
    int N   = (argc > 1) ? atoi(argv[1]) : 20000;
    int TAM = (argc > 2) ? atoi(argv[2]) : 64;
    LARGE_INTEGER freq, a, b;
    char maquina[256] = "";
    DWORD mlen = sizeof(maquina);
    HKEY h;
    DWORD disp;
    unsigned char *buf, *lido;
    char nm[32];
    double w_lazy, w_dur, rd;
    int i, ok = 1;
    LONG r;

    if (N < 1) N = 1;
    if (TAM < 1) TAM = 1;
    if (TAM > 65536) TAM = 65536;

    QueryPerformanceFrequency(&freq);
    GetComputerNameA(maquina, &mlen);

    r = RegCreateKeyExA(HKEY_CURRENT_USER, SUBCHAVE, 0, NULL,
                        REG_OPTION_NON_VOLATILE, KEY_READ | KEY_WRITE,
                        NULL, &h, &disp);
    if (r != ERROR_SUCCESS) {
        fprintf(stderr, "RegCreateKeyEx falhou: %ld\n", r);
        return 2;
    }

    buf  = (unsigned char *)malloc(TAM);
    lido = (unsigned char *)malloc(TAM);
    if (!buf || !lido) {
        fprintf(stderr, "sem memoria\n");
        return 2;
    }

    /* fase 0 -- cria as N chaves uma vez (aquece a colmeia) */
    for (i = 0; i < N; i++) {
        nome(nm, sizeof(nm), i);
        encher(buf, TAM, i);
        RegSetValueExA(h, nm, 0, REG_BINARY, buf, TAM);
    }

    /* fase 1 -- escrita PREGUICOSA: N atualizacoes, sem flush */
    QueryPerformanceCounter(&a);
    for (i = 0; i < N; i++) {
        nome(nm, sizeof(nm), i);
        encher(buf, TAM, i);
        RegSetValueExA(h, nm, 0, REG_BINARY, buf, TAM);
    }
    QueryPerformanceCounter(&b);
    w_lazy = us(b.QuadPart - a.QuadPart, freq.QuadPart) / N;

    /* fase 2 -- escrita DURAVEL: RegFlushKey a cada chave (colmeia inteira) */
    QueryPerformanceCounter(&a);
    for (i = 0; i < N; i++) {
        nome(nm, sizeof(nm), i);
        encher(buf, TAM, i);
        RegSetValueExA(h, nm, 0, REG_BINARY, buf, TAM);
        RegFlushKey(h);
    }
    QueryPerformanceCounter(&b);
    w_dur = us(b.QuadPart - a.QuadPart, freq.QuadPart) / N;

    /* fase 3 -- leitura (colmeia quente) */
    QueryPerformanceCounter(&a);
    for (i = 0; i < N; i++) {
        DWORD tipo, tam = TAM;
        nome(nm, sizeof(nm), i);
        RegQueryValueExA(h, nm, NULL, &tipo, lido, &tam);
    }
    QueryPerformanceCounter(&b);
    rd = us(b.QuadPart - a.QuadPart, freq.QuadPart) / N;

    /* prova real -- le de volta e confere o padrao de CADA chave */
    for (i = 0; i < N && ok; i++) {
        DWORD tipo, tam = TAM;
        nome(nm, sizeof(nm), i);
        encher(buf, TAM, i);
        if (RegQueryValueExA(h, nm, NULL, &tipo, lido, &tam) != ERROR_SUCCESS) {
            ok = 0;
        } else if (tam != (DWORD)TAM || memcmp(buf, lido, TAM) != 0) {
            ok = 0;
        }
    }

    /* limpa: apaga a subarvore inteira que criou -- nao deixa lixo na
       colmeia do usuario, do mesmo modo que a bateria da casa nao deixa
       diretorio solto em /tmp. */
    RegCloseKey(h);
    RegDeleteTreeA(HKEY_CURRENT_USER, SUBCHAVE);

    printf("# PhxSql bancada/registro -- Registro do Windows, latencia por ponto\n");
    printf("# maquina: %s | QPC: %lld Hz | N=%d | valor=%d bytes\n",
           maquina, (long long)freq.QuadPart, N, TAM);
    printf("escrita_preguicosa_us_op: %.3f\n", w_lazy);
    printf("escrita_duravel_us_op:    %.3f   (RegFlushKey por chave -- reescreve a colmeia)\n", w_dur);
    printf("leitura_us_op:            %.3f   (colmeia quente, em RAM)\n", rd);
    printf("verificacao: %s\n", ok ? "OK" : "FALHOU");

    free(buf);
    free(lido);
    if (!ok) {
        fprintf(stderr, "verificacao FALHOU -- nao publique estes numeros\n");
        return 1;
    }
    return 0;
}
