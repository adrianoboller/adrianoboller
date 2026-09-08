<?php
// CRUD de clientes -- procedural, como foi escrito em 2011.
require_once __DIR__ . '/db.php';

// BR-001: e-mail e unico e guardado em minusculas, sem espacos nas pontas.
function normalizar_email($email) {
    return strtolower(trim($email));
}

// BR-002: limite de credito nunca e negativo e vai com 2 casas (arredonda "meio para cima").
function normalizar_limite($valor) {
    $v = round((float)$valor, 2);
    if ($v < 0) { return array('erro' => 'limite de credito nao pode ser negativo'); }
    return $v;
}

// BR-003: nome obrigatorio, entre 3 e 80 caracteres, sem espaco duplo.
function normalizar_nome($nome) {
    $n = preg_replace('/\s+/', ' ', trim($nome));
    if (strlen($n) < 3 || strlen($n) > 80) { return array('erro' => 'nome deve ter entre 3 e 80 caracteres'); }
    return $n;
}

function listar($so_ativos = true) {
    $cx = conectar();
    $sql = 'SELECT id, nome, email, limite_credito, ativo FROM clientes'
         . ($so_ativos ? ' WHERE ativo = 1' : '') . ' ORDER BY nome, id';
    $r = mysqli_query($cx, $sql);
    $saida = array();
    while ($l = mysqli_fetch_assoc($r)) {
        $l['id'] = (int)$l['id']; $l['ativo'] = (int)$l['ativo'];
        $l['limite_credito'] = number_format((float)$l['limite_credito'], 2, '.', '');
        $saida[] = $l;
    }
    return $saida;
}

function incluir($nome, $email, $limite) {
    $cx = conectar();
    $n = normalizar_nome($nome);     if (is_array($n)) return $n;
    $e = normalizar_email($email);
    $l = normalizar_limite($limite); if (is_array($l)) return $l;
    $st = mysqli_prepare($cx, 'INSERT INTO clientes (nome, email, limite_credito) VALUES (?, ?, ?)');
    mysqli_stmt_bind_param($st, 'ssd', $n, $e, $l);
    if (!mysqli_stmt_execute($st)) {
        // BR-001 na pratica: a chave unica do banco e a segunda linha de defesa
        if (mysqli_errno($cx) == 1062) return array('erro' => 'e-mail ja cadastrado');
        return array('erro' => mysqli_error($cx));
    }
    return array('id' => mysqli_insert_id($cx));
}

function alterar($id, $nome, $email, $limite) {
    $cx = conectar();
    $n = normalizar_nome($nome);     if (is_array($n)) return $n;
    $e = normalizar_email($email);
    $l = normalizar_limite($limite); if (is_array($l)) return $l;
    $st = mysqli_prepare($cx, 'UPDATE clientes SET nome = ?, email = ?, limite_credito = ? WHERE id = ? AND ativo = 1');
    mysqli_stmt_bind_param($st, 'ssdi', $n, $e, $l, $id);
    if (!mysqli_stmt_execute($st)) {
        if (mysqli_errno($cx) == 1062) return array('erro' => 'e-mail ja cadastrado');
        return array('erro' => mysqli_error($cx));
    }
    return array('alterados' => mysqli_stmt_affected_rows($st));
}

// BR-004: excluir NAO apaga -- desativa. Historico de venda aponta para o cliente.
function excluir($id) {
    $cx = conectar();
    $st = mysqli_prepare($cx, 'UPDATE clientes SET ativo = 0 WHERE id = ? AND ativo = 1');
    mysqli_stmt_bind_param($st, 'i', $id);
    mysqli_stmt_execute($st);
    return array('desativados' => mysqli_stmt_affected_rows($st));
}
