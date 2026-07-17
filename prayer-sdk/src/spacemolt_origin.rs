pub const DEFAULT_SPACEMOLT_ORIGIN: &str = "https://game.spacemolt.com";

pub fn normalize_origin(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() {
        DEFAULT_SPACEMOLT_ORIGIN.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_origin_points_to_game_spacemolt() {
        assert_eq!(normalize_origin(""), DEFAULT_SPACEMOLT_ORIGIN);
    }

    #[test]
    fn custom_https_origin_is_normalized() {
        assert_eq!(
            normalize_origin("https://example.test/"),
            "https://example.test"
        );
    }

    #[test]
    fn custom_http_origin_is_preserved() {
        assert_eq!(
            normalize_origin("http://localhost:3000"),
            "http://localhost:3000"
        );
    }
}
