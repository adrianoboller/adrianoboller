<?php
// Sistema FATURAMENTO - regras de negocio
// Autor original: equipe interna, 2009. Mantido desde entao sem framework.
// ATENCAO: este arquivo concentra as regras que o financeiro cobra. Nao mexer
// sem falar com o Marcelo (financeiro).

define('MULTA_ATRASO_PCT', 2.0);      // BR-101
define('JUROS_MES_PCT', 1.0);         // BR-101
define('DESCONTO_AVISTA_PCT', 5.0);   // BR-102
define('DIAS_INADIMPLENCIA', 30);     // BR-103
define('TETO_PARCELAS', 12);          // BR-104

/**
 * BR-101 - Multa e juros de titulo em atraso.
 * Multa fixa de 2% sobre o valor, mais 1% ao mes pro rata die, contando a
 * partir do dia SEGUINTE ao vencimento. Titulo em dia nao tem acrescimo.
 * O arredondamento e sempre para 2 casas, meio para cima.
 */
function calcula_encargos($valor, $vencimento, $data_pagamento) {
    $v = strtotime($vencimento);
    $p = strtotime($data_pagamento);
    $dias = (int) floor(($p - $v) / 86400);
    if ($dias <= 0) {
        return array('multa' => 0.0, 'juros' => 0.0, 'total' => round($valor, 2));
    }
    $multa = $valor * MULTA_ATRASO_PCT / 100;
    $juros = $valor * (JUROS_MES_PCT / 100) * ($dias / 30);
    return array(
        'multa' => round($multa, 2),
        'juros' => round($juros, 2),
        'total' => round($valor + $multa + $juros, 2),
    );
}

/**
 * BR-102 - Desconto por forma de pagamento.
 * A vista ("AV") ganha 5%. A prazo ("PZ") nao ganha nada. Boleto ("BO") segue
 * a regra do a prazo. Forma desconhecida NAO ganha desconto - decisao de 2014
 * depois do caso da filial de Joinville que cadastrou forma nova sozinha.
 */
function desconto_por_forma($subtotal, $forma) {
    if ($forma === 'AV') {
        return round($subtotal * DESCONTO_AVISTA_PCT / 100, 2);
    }
    return 0.0;
}

/**
 * BR-103 - Bloqueio por inadimplencia.
 * Cliente com titulo vencido ha mais de 30 dias nao fatura. Cliente com
 * titulo vencido ha 30 dias ou menos fatura, mas o sistema avisa.
 * Retorna 'BLOQUEADO', 'AVISO' ou 'LIVRE'.
 */
function situacao_do_cliente($maior_atraso_em_dias) {
    if ($maior_atraso_em_dias > DIAS_INADIMPLENCIA) {
        return 'BLOQUEADO';
    }
    if ($maior_atraso_em_dias > 0) {
        return 'AVISO';
    }
    return 'LIVRE';
}

/**
 * BR-104 - Parcelamento.
 * Parcelas iguais a cada 30 dias. A diferenca de arredondamento vai toda na
 * ULTIMA parcela, para a soma bater com o total ao centavo.
 * Acima de 12 parcelas o sistema recusa.
 */
function gera_parcelas($total, $n, $primeira_data) {
    if ($n < 1 || $n > TETO_PARCELAS) {
        return false;
    }
    $centavos = (int) round($total * 100);
    $base = intdiv($centavos, $n);
    $parcelas = array();
    $soma = 0;
    for ($i = 1; $i <= $n; $i++) {
        $c = ($i < $n) ? $base : ($centavos - $soma);
        $soma += $base;
        $parcelas[] = array(
            'numero' => $i,
            'valor' => $c / 100,
            'vencimento' => date('Y-m-d', strtotime($primeira_data . ' +' . (($i - 1) * 30) . ' days')),
        );
    }
    return $parcelas;
}

/**
 * BR-105 - CNPJ valido pelos dois digitos verificadores.
 * Aceita com ou sem mascara. CNPJ com todos os digitos iguais e invalido.
 */
function valida_cnpj($cnpj) {
    $cnpj = preg_replace('/[^0-9]/', '', $cnpj);
    if (strlen($cnpj) != 14 || preg_match('/^(\d)\1{13}$/', $cnpj)) {
        return false;
    }
    for ($t = 12; $t < 14; $t++) {
        $soma = 0;
        $peso = ($t == 12) ? 5 : 6;
        for ($i = 0; $i < $t; $i++) {
            $soma += $cnpj[$i] * $peso;
            $peso = ($peso == 2) ? 9 : $peso - 1;
        }
        $d = ($soma % 11 < 2) ? 0 : 11 - ($soma % 11);
        if ($cnpj[$t] != $d) {
            return false;
        }
    }
    return true;
}
