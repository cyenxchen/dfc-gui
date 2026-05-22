//! Helpers for user-entered table filters.

const FILTER_VALUE_SEPARATORS: [char; 6] = ['\n', '\r', ',', '，', ';', '；'];

pub fn split_filter_values(value: &str) -> Vec<String> {
    value
        .split(|ch| FILTER_VALUE_SEPARATORS.contains(&ch))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
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
}
