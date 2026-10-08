//! URL query flags (`?webgl`, `?sample`).

/// Whether `search` (the page's `location.search`, with or without its leading `?`) holds the
/// flag `name`: a parameter whose key is exactly `name`, with or without a value.
pub fn has_flag(search: &str, name: &str) -> bool {
    let search = search.strip_prefix('?').unwrap_or(search);
    search.split('&').any(|param| param.split_once('=').map_or(param, |(key, _)| key) == name)
}

#[cfg(test)]
mod tests {
    use super::has_flag;

    #[test]
    fn a_flag_is_a_parameter_key() {
        for q in ["?sample", "?webgl&sample", "?sample=1", "sample", "?sample&webgl", "?a=b&sample="] {
            assert!(has_flag(q, "sample"), "{q}");
        }
        for q in ["", "?", "?note=sample", "?note=not_a_sample", "?samples", "?xsample", "?webgl", "?note=a&sample_size=3"] {
            assert!(!has_flag(q, "sample"), "{q}");
        }
        assert!(has_flag("?webgl&sample", "webgl"));
        assert!(!has_flag("?nowebgl", "webgl"));
    }
}
