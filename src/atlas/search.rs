//! Palette matching. Pure text logic: fold accents, rank titles over bodies.

/// Lowercase and strip the accents people do not type, so "schrodinger" finds
/// "Schrödinger". Maps one char to one char, so indices line up with the input.
pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'Ä' | 'Á' | 'À' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'É' | 'È' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'Ö' | 'Ó' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ü' | 'Ú' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            '\u{2013}' | '\u{2014}' => '-',
            other => other.to_lowercase().next().unwrap_or(other),
        })
        .collect()
}

pub fn tokens(query: &str) -> Vec<String> {
    fold(query).split_whitespace().map(str::to_string).collect()
}

fn is_word_start(text: &str, byte_at: usize) -> bool {
    byte_at == 0
        || text[..byte_at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric())
}

fn best_hit(token: &str, text: &str) -> Option<i32> {
    let mut best = None;
    for (at, _) in text.match_indices(token) {
        let hit = if is_word_start(text, at) { 2 } else { 1 };
        best = Some(best.map_or(hit, |b: i32| b.max(hit)));
    }
    best
}

fn is_subsequence(token: &str, text: &str) -> bool {
    let mut chars = text.chars();
    token.chars().all(|t| chars.by_ref().any(|c| c == t))
}

/// Rank a candidate, or None if any token fails to match. Every token must hit
/// somewhere. Word starts beat substrings, and the title beats the body.
pub fn score(tokens: &[String], title: &str, body: &str) -> Option<i32> {
    let mut total = 0;
    for token in tokens {
        let title_hit = best_hit(token, title);
        total += match title_hit {
            Some(2) => 100,
            Some(_) => 60,
            None => match best_hit(token, body) {
                Some(2) => 25,
                Some(_) => 12,
                None if token.chars().count() >= 3 && is_subsequence(token, title) => 5,
                None => return None,
            },
        };
    }
    // Among equals, the shorter title is the more exact match.
    Some(total * 10 - title.chars().count().min(60) as i32)
}

/// Char indices of the title to highlight for these tokens.
pub fn highlights(tokens: &[String], title: &str) -> Vec<usize> {
    let folded = fold(title);
    let mut out = Vec::new();
    for token in tokens {
        if let Some(byte_at) = folded.find(token.as_str()) {
            let start = folded[..byte_at].chars().count();
            out.extend(start..start + token.chars().count());
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(q: &str, title: &str, body: &str) -> Option<i32> {
        score(&tokens(q), &fold(title), &fold(body))
    }

    #[test]
    fn accents_fold_one_to_one() {
        assert_eq!(fold("Schrödinger"), "schrodinger");
        assert_eq!(fold("Rössler").chars().count(), "Rössler".chars().count());
    }

    #[test]
    fn title_beats_body_and_word_start_beats_substring() {
        let title = s("wave", "Wave packet", "x").unwrap();
        let inner = s("wave", "Shockwave", "x").unwrap();
        let body = s("wave", "Packet", "a wave").unwrap();
        assert!(title > inner && inner > body);
    }

    #[test]
    fn all_tokens_must_match_somewhere() {
        assert!(s("lorenz chaos", "Lorenz swarm", "chaos keeps its shape").is_some());
        assert!(s("lorenz quark", "Lorenz swarm", "chaos keeps its shape").is_none());
    }

    #[test]
    fn subsequence_rescues_a_terse_query_only_in_the_title() {
        assert!(s("brch", "Brachistochrone Race", "").is_some());
        assert!(s("brch", "Other", "Brachistochrone Race").is_none());
    }

    #[test]
    fn empty_query_matches_everything() {
        assert!(s("", "Anything", "").is_some());
    }

    #[test]
    fn highlights_point_at_the_match_in_the_original_title() {
        assert_eq!(
            highlights(&tokens("schro"), "Schrödinger"),
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(highlights(&tokens("ger"), "Schrödinger"), vec![8, 9, 10]);
        assert!(highlights(&tokens("zzz"), "Schrödinger").is_empty());
    }
}
