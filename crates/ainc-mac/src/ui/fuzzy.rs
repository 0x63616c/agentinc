//! A small fuzzy matcher for the command palette and pickers.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyMatch {
    pub score: i32,
    /// Character positions in the candidate that matched the query.
    pub positions: Vec<usize>,
}

fn is_boundary(previous: Option<char>) -> bool {
    previous.is_none_or(|c| !c.is_alphanumeric())
}

/// Matches `query` as a subsequence of `candidate`, case-insensitively.
/// Consecutive characters, word starts and prefixes score higher; every
/// candidate matches an empty query with a zero score.
pub fn fuzzy_match(query: &str, candidate: &str) -> Option<FuzzyMatch> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            positions: vec![],
        });
    }
    let candidate: Vec<char> = candidate.chars().collect();
    let lowered: Vec<char> = candidate.iter().flat_map(|c| c.to_lowercase()).collect();
    if lowered.len() != candidate.len() {
        return fuzzy_match_slow(&query, &candidate);
    }
    let mut positions = Vec::with_capacity(query.len());
    let mut score = 0;
    let mut cursor = 0;
    let mut previous_match: Option<usize> = None;
    for needle in &query {
        let found = (cursor..lowered.len()).find(|&index| lowered[index] == *needle)?;
        let boundary = is_boundary(found.checked_sub(1).map(|i| candidate[i]));
        score += 1;
        if boundary {
            score += 8;
        }
        if previous_match == Some(found.wrapping_sub(1)) {
            score += 6;
        }
        if found == 0 {
            score += 12;
        }
        score -= (found - cursor).min(10) as i32;
        positions.push(found);
        previous_match = Some(found);
        cursor = found + 1;
    }
    // Shorter candidates that consume more of their text rank higher.
    score += (20 - candidate.len().min(20)) as i32 / 2;
    Some(FuzzyMatch { score, positions })
}

fn fuzzy_match_slow(query: &[char], candidate: &[char]) -> Option<FuzzyMatch> {
    let mut positions = Vec::with_capacity(query.len());
    let mut cursor = 0;
    for needle in query {
        let found = (cursor..candidate.len()).find(|&index| {
            candidate[index].to_lowercase().next() == needle.to_lowercase().next()
        })?;
        positions.push(found);
        cursor = found + 1;
    }
    Some(FuzzyMatch {
        score: positions.len() as i32,
        positions,
    })
}

#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn empty_query_matches_everything_with_zero_score() {
        let m = fuzzy_match("", "Settings").unwrap();
        assert_eq!(m.score, 0);
        assert!(m.positions.is_empty());
    }

    #[test]
    fn subsequence_matches_are_case_insensitive() {
        let m = fuzzy_match("stg", "Settings").unwrap();
        assert_eq!(m.positions, vec![0, 2, 6]);
        assert!(fuzzy_match("xyz", "Settings").is_none());
    }

    #[test]
    fn prefixes_and_word_starts_outrank_scattered_matches() {
        let prefix = fuzzy_match("tick", "Tickets").unwrap().score;
        let scattered = fuzzy_match("tick", "Automatic check").unwrap().score;
        assert!(prefix > scattered);
        let word_start = fuzzy_match("tf", "Toggle Font").unwrap().score;
        let inside = fuzzy_match("tf", "Notification").unwrap().score;
        assert!(word_start > inside);
    }

    #[test]
    fn whitespace_in_the_query_is_ignored() {
        assert!(fuzzy_match("new work", "New workspace").is_some());
        assert!(fuzzy_match("  ", "Anything").is_some());
    }

    #[test]
    fn multibyte_candidates_still_match() {
        let m = fuzzy_match("cafb", "Café bar").unwrap();
        assert_eq!(m.positions, vec![0, 1, 2, 5]);
        assert!(fuzzy_match("cafe", "Café bar").is_none());
        let m = fuzzy_match("iş", "İş").unwrap();
        assert_eq!(m.positions.len(), 2);
    }
}
