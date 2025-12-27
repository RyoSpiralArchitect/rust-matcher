use regex::Regex;
use std::{fs, path::PathBuf};

#[test]
fn cdn_assets_must_use_sri() {
    let index = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../gui/index.html");

    let Ok(contents) = fs::read_to_string(&index) else {
        // GUI is optional in some environments; skip if missing.
        return;
    };

    let tag_re =
        Regex::new(r#"<(?:script|link)[^>]+(?:src|href)=\"https?://[^\">]+\"[^>]*>"#).unwrap();

    for tag in tag_re.find_iter(&contents) {
        assert!(
            tag.as_str().contains("integrity="),
            "External asset missing integrity attribute: {}",
            tag.as_str()
        );
    }
}
