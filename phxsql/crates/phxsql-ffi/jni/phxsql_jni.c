/* phxsql_jni.c -- wrapper JNI minimo para Android
 *
 * Expoe a ABI de C do PhxSql para Android via JNI.
 * Compilado junto com libphxsql_ffi para a APK.
 */

#include <jni.h>
#include <string.h>
#include "../include/phxsql.h"

/* Callback de teste: roda a prova em um diretorio temporario */
JNIEXPORT jint JNICALL Java_br_phxsql_PhxSql_testar
  (JNIEnv *env, jclass cls, jstring dir_bytes) {

    const char *dir = (*env)->GetStringUTFChars(env, dir_bytes, NULL);
    if (!dir) return -1;

    /* Abre a base de dados */
    PhxBase *base = NULL;
    int32_t r = phx_base_abrir((const uint8_t *)dir, strlen(dir),
                               (const uint8_t *)"teste", 5,
                               PHX_CRIAR, &base);

    (*env)->ReleaseStringUTFChars(env, dir_bytes, dir);

    if (r != PHX_OK) return r;

    /* Fecha */
    phx_base_fechar(base);
    return PHX_OK;
}

/* Retorna a versao do PhxSql */
JNIEXPORT jstring JNICALL Java_br_phxsql_PhxSql_versao
  (JNIEnv *env, jclass cls) {

    uint8_t buf[64];
    size_t precisa = 0;
    phx_versao(buf, sizeof buf, &precisa);

    if (precisa >= sizeof buf) precisa = sizeof buf - 1;
    buf[precisa] = 0;

    return (*env)->NewStringUTF(env, (const char *)buf);
}
