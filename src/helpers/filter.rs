//! Helpers for user-entered table filters.

const FILTER_VALUE_SEPARATORS: [char; 6] = ['\n', '\r', ',', '，', ';', '；'];

/// Iterate the trimmed, non-empty filter values in `value`. Single source of truth for how a
/// filter string is broken into individual values; [`split_filter_values`] and
/// [`count_filter_values`] are both built on it.
fn filter_value_parts(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|ch| FILTER_VALUE_SEPARATORS.contains(&ch))
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

pub fn split_filter_values(value: &str) -> Vec<String> {
    filter_value_parts(value).map(ToString::to_string).collect()
}

/// Count the filter values in `value` without allocating. Kept in sync with
/// [`split_filter_values`] via [`filter_value_parts`]; use this on hot render paths where only
/// the count is needed.
pub fn count_filter_values(value: &str) -> usize {
    filter_value_parts(value).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_filter_values_accepts_common_separators() {
        assert_eq!(
            split_filter_values(" 100,200；300\n400 "),
            vec!["100", "200", "300", "400"]
        );
    }

    #[test]
    fn count_filter_values_matches_split_len() {
        for value in [
            "",
            "   ",
            ",;，；",
            "100",
            " 100,200；300\n400 ",
            "\n100\n\n200\n",
        ] {
            assert_eq!(
                count_filter_values(value),
                split_filter_values(value).len(),
                "count mismatch for {value:?}"
            );
        }
    }
}
