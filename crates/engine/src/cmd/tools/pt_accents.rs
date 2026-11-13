//! Portuguese accents added as you type: "nao " becomes "não ", "procuracao " "procuração ".
//!
//! A word typed without its accent or cedilla is put right when the space (or punctuation) after
//! it is typed. Two sources, both Portuguese-only:
//! - [`ACCENTS`]: our own list of very common words whose unaccented spelling is not itself a
//!   word, so the fix is never wrong ("esta" and "pais" are real words and are not in it);
//! - the installed Portuguese dictionary: a misspelled word with exactly one suggestion that
//!   differs from it only by accents and cedillas is replaced by that suggestion.

use wordcraft_proof::ProofLang;

/// Unaccented spelling (lowercase) and the accented one.
const ACCENTS: &[(&str, &str)] = &[
    ("nao", "não"),
    ("tambem", "também"),
    ("voce", "você"),
    ("voces", "vocês"),
    ("sao", "são"),
    ("estao", "estão"),
    ("serao", "serão"),
    ("entao", "então"),
    ("ate", "até"),
    ("ja", "já"),
    ("ha", "há"),
    ("alem", "além"),
    ("apos", "após"),
    ("porem", "porém"),
    ("atraves", "através"),
    ("ninguem", "ninguém"),
    ("alguem", "alguém"),
    ("tres", "três"),
    ("mes", "mês"),
    ("ola", "olá"),
    ("alias", "aliás"),
    ("sera", "será"),
    ("podera", "poderá"),
    ("devera", "deverá"),
    ("havera", "haverá"),
    ("estara", "estará"),
    ("ficara", "ficará"),
    ("mae", "mãe"),
    ("maes", "mães"),
    ("irmao", "irmão"),
    ("irmaos", "irmãos"),
    ("orgao", "órgão"),
    ("orgaos", "órgãos"),
    ("coracao", "coração"),
    ("coracoes", "corações"),
    ("area", "área"),
    ("areas", "áreas"),
    ("agua", "água"),
    ("familia", "família"),
    ("historia", "história"),
    ("memoria", "memória"),
    ("lingua", "língua"),
    ("pagina", "página"),
    ("paginas", "páginas"),
    ("numero", "número"),
    ("numeros", "números"),
    ("codigo", "código"),
    ("codigos", "códigos"),
    ("titulo", "título"),
    ("titulos", "títulos"),
    ("periodo", "período"),
    ("metodo", "método"),
    ("portugues", "português"),
    ("ingles", "inglês"),
    ("frances", "francês"),
    ("duvida", "dúvida"),
    ("duvidas", "dúvidas"),
    ("ferias", "férias"),
    ("util", "útil"),
    ("facil", "fácil"),
    ("dificil", "difícil"),
    ("possivel", "possível"),
    ("impossivel", "impossível"),
    ("responsavel", "responsável"),
    ("disponivel", "disponível"),
    ("unico", "único"),
    ("unica", "única"),
    ("ultimo", "último"),
    ("ultima", "última"),
    ("proximo", "próximo"),
    ("proxima", "próxima"),
    ("minimo", "mínimo"),
    ("maximo", "máximo"),
    ("basico", "básico"),
    ("basica", "básica"),
    ("especifico", "específico"),
    ("historico", "histórico"),
    ("automatico", "automático"),
    ("automatica", "automática"),
    ("gramatica", "gramática"),
    ("juridico", "jurídico"),
    ("juridica", "jurídica"),
    ("judiciario", "judiciário"),
    ("salario", "salário"),
    ("salarios", "salários"),
    ("horario", "horário"),
    ("horarios", "horários"),
    ("contrario", "contrário"),
    ("necessario", "necessário"),
    ("necessaria", "necessária"),
    ("ordinario", "ordinário"),
    ("acao", "ação"),
    ("acoes", "ações"),
    ("informacao", "informação"),
    ("informacoes", "informações"),
    ("solucao", "solução"),
    ("solucoes", "soluções"),
    ("situacao", "situação"),
    ("situacoes", "situações"),
    ("educacao", "educação"),
    ("relacao", "relação"),
    ("relacoes", "relações"),
    ("funcao", "função"),
    ("funcoes", "funções"),
    ("opcao", "opção"),
    ("opcoes", "opções"),
    ("producao", "produção"),
    ("nacao", "nação"),
    ("nacoes", "nações"),
    ("decisao", "decisão"),
    ("decisoes", "decisões"),
    ("razao", "razão"),
    ("razoes", "razões"),
    ("atencao", "atenção"),
    ("condicao", "condição"),
    ("condicoes", "condições"),
    ("posicao", "posição"),
    ("organizacao", "organização"),
    ("administracao", "administração"),
    ("comunicacao", "comunicação"),
    ("legislacao", "legislação"),
    ("populacao", "população"),
    ("operacao", "operação"),
    ("operacoes", "operações"),
    ("aplicacao", "aplicação"),
    ("avaliacao", "avaliação"),
    ("atualizacao", "atualização"),
    ("configuracao", "configuração"),
    ("instalacao", "instalação"),
    ("selecao", "seleção"),
    ("protecao", "proteção"),
    ("correcao", "correção"),
    ("descricao", "descrição"),
    ("versao", "versão"),
    ("versoes", "versões"),
    ("sessao", "sessão"),
    ("sessoes", "sessões"),
    ("secao", "seção"),
    ("secoes", "seções"),
    ("excecao", "exceção"),
    ("excecoes", "exceções"),
    ("conclusao", "conclusão"),
    ("conclusoes", "conclusões"),
    ("impressao", "impressão"),
    ("expressao", "expressão"),
    ("permissao", "permissão"),
    ("permissoes", "permissões"),
    ("comissao", "comissão"),
    ("reuniao", "reunião"),
    ("reunioes", "reuniões"),
    ("uniao", "união"),
    ("opiniao", "opinião"),
    ("opinioes", "opiniões"),
    ("procuracao", "procuração"),
    ("procuracoes", "procurações"),
    ("obrigacao", "obrigação"),
    ("obrigacoes", "obrigações"),
    ("pensao", "pensão"),
    ("pensoes", "pensões"),
    ("execucao", "execução"),
    ("citacao", "citação"),
    ("intimacao", "intimação"),
    ("peticao", "petição"),
    ("peticoes", "petições"),
    ("audiencia", "audiência"),
    ("sentenca", "sentença"),
    ("competencia", "competência"),
    ("jurisdicao", "jurisdição"),
    ("jurisprudencia", "jurisprudência"),
    ("indenizacao", "indenização"),
    ("rescisao", "rescisão"),
    ("rescisoes", "rescisões"),
    ("remuneracao", "remuneração"),
    ("licenca", "licença"),
    ("agencia", "agência"),
    ("experiencia", "experiência"),
    ("ciencia", "ciência"),
    ("frequencia", "frequência"),
    ("consequencia", "consequência"),
    ("ausencia", "ausência"),
    ("presenca", "presença"),
    ("diferenca", "diferença"),
    ("forca", "força"),
    ("servico", "serviço"),
    ("servicos", "serviços"),
    ("preco", "preço"),
    ("precos", "preços"),
    ("endereco", "endereço"),
    ("enderecos", "endereços"),
    ("comeco", "começo"),
];

/// `word` with its accents and cedillas removed, in lowercase.
fn plain(word: &str) -> String {
    word.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// `rep` (lowercase) in the case `word` was typed in: "Nao" → "Não", "NAO" → "NÃO".
fn in_case_of(word: &str, rep: &str) -> String {
    let mut letters = word.chars().filter(|c| c.is_alphabetic());
    let first_upper = letters.next().is_some_and(char::is_uppercase);
    let rest_upper = word.chars().filter(|c| c.is_alphabetic()).count() > 1 && letters.all(char::is_uppercase);
    if first_upper && rest_upper {
        rep.to_uppercase()
    } else if first_upper {
        let mut c = rep.chars();
        c.next().map(|x| x.to_uppercase().chain(c).collect()).unwrap_or_default()
    } else {
        rep.to_string()
    }
}

/// The word with its accents put in, when `word` is Portuguese typed without them. `lang` is the
/// language the word is in. `None` when the word is fine, unknown, or already accented.
pub(super) fn accent_fix(word: &str, lang: ProofLang) -> Option<String> {
    if word.chars().count() < 2 || !word.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let lower = word.to_ascii_lowercase();
    if let Some((_, rep)) = ACCENTS.iter().find(|(a, _)| *a == lower) {
        return Some(in_case_of(word, rep));
    }
    // Something the list doesn't know: the dictionary's accent-only suggestion, if it is clear.
    if word.chars().count() < 3 || wordcraft_proof::is_correct_in(word, lang) {
        return None;
    }
    let mut found: Option<String> = None;
    for cand in wordcraft_proof::suggest_in(word, 8, lang) {
        if cand.to_lowercase() != lower && plain(&cand) == lower {
            match &found {
                Some(f) if *f != cand => return None,
                _ => found = Some(cand),
            }
        }
    }
    found.map(|f| in_case_of(word, &f.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_has_no_real_words_and_matches() {
        for (plain_form, accented) in ACCENTS {
            assert_eq!(&plain(accented), plain_form, "{accented}");
            assert!(plain_form.chars().all(|c| c.is_ascii_lowercase()));
        }
    }

    #[test]
    fn keeps_the_case_typed() {
        assert_eq!(accent_fix("nao", ProofLang::Pt).as_deref(), Some("não"));
        assert_eq!(accent_fix("Nao", ProofLang::Pt).as_deref(), Some("Não"));
        assert_eq!(accent_fix("PROCURACAO", ProofLang::Pt).as_deref(), Some("PROCURAÇÃO"));
        assert_eq!(accent_fix("outorgante", ProofLang::Pt), None);
        assert_eq!(accent_fix("esta", ProofLang::Pt), None);
    }
}
