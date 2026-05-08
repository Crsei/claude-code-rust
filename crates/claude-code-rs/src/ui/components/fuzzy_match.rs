#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FuzzyMatchKind {
    Empty,
    Exact,
    Prefix,
    Contains,
    Subsequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzyMatch {
    pub kind: FuzzyMatchKind,
    pub score: usize,
}

pub fn fuzzy_match(candidate: &str, query: &str) -> Option<FuzzyMatch> {
    let query = normalize(query);
    if query.is_empty() {
        return Some(FuzzyMatch {
            kind: FuzzyMatchKind::Empty,
            score: 0,
        });
    }

    let candidate = normalize(candidate);
    if candidate == query {
        return Some(FuzzyMatch {
            kind: FuzzyMatchKind::Exact,
            score: 0,
        });
    }

    if candidate.starts_with(&query) {
        return Some(FuzzyMatch {
            kind: FuzzyMatchKind::Prefix,
            score: 100 + candidate.len().saturating_sub(query.len()),
        });
    }

    if let Some(index) = candidate.find(&query) {
        return Some(FuzzyMatch {
            kind: FuzzyMatchKind::Contains,
            score: 200 + index + candidate.len().saturating_sub(query.len()),
        });
    }

    subsequence_score(&candidate, &query).map(|score| FuzzyMatch {
        kind: FuzzyMatchKind::Subsequence,
        score: 300 + score,
    })
}

pub fn best_fuzzy_match<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    query: &str,
) -> Option<FuzzyMatch> {
    candidates
        .into_iter()
        .filter_map(|candidate| fuzzy_match(candidate, query))
        .min_by_key(|matched| matched.score)
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn subsequence_score(candidate: &str, query: &str) -> Option<usize> {
    let candidate_chars = candidate.chars().collect::<Vec<_>>();
    let query_chars = query.chars().collect::<Vec<_>>();
    let mut positions = Vec::with_capacity(query_chars.len());
    let mut cursor = 0usize;

    for query_char in query_chars {
        let found = candidate_chars
            .iter()
            .enumerate()
            .skip(cursor)
            .find_map(|(idx, candidate_char)| (*candidate_char == query_char).then_some(idx))?;
        positions.push(found);
        cursor = found + 1;
    }

    let first = *positions.first()?;
    let last = *positions.last()?;
    let span = last.saturating_sub(first) + 1;
    let gaps = span.saturating_sub(positions.len());
    Some(gaps + first + candidate_chars.len().saturating_sub(positions.len()))
}

#[cfg(test)]
mod tests {
    use super::{best_fuzzy_match, fuzzy_match, FuzzyMatchKind};

    #[test]
    fn ranks_exact_prefix_contains_then_subsequence() {
        assert_eq!(
            fuzzy_match("mcp", "mcp").map(|m| m.kind),
            Some(FuzzyMatchKind::Exact)
        );
        assert_eq!(
            fuzzy_match("memory", "mem").map(|m| m.kind),
            Some(FuzzyMatchKind::Prefix)
        );
        assert_eq!(
            fuzzy_match("open-memory", "mem").map(|m| m.kind),
            Some(FuzzyMatchKind::Contains)
        );
        assert_eq!(
            fuzzy_match("model context protocol", "mcp").map(|m| m.kind),
            Some(FuzzyMatchKind::Subsequence)
        );
        assert!(fuzzy_match("status", "xyz").is_none());
    }

    #[test]
    fn best_match_uses_the_lowest_ranked_candidate() {
        let candidates = ["model context protocol", "mcp", "manage command palette"];
        let matched = best_fuzzy_match(candidates, "mcp").expect("match");

        assert_eq!(matched.kind, FuzzyMatchKind::Exact);
        assert_eq!(matched.score, 0);
    }
}
