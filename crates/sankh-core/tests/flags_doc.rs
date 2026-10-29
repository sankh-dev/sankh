//! The site's flag page is generated from the flag table; this keeps it current.

use std::path::Path;

#[test]
fn curl_flags_page_is_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/src/content/curl-flags.md");
    let want = sankh_core::parse::flags::markdown();
    if std::env::var_os("SANKH_UPDATE_DOCS").is_some() {
        std::fs::write(&path, &want).unwrap();
        return;
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == want,
        "{} is out of date; run `SANKH_UPDATE_DOCS=1 cargo test -p sankh-core --test flags_doc`",
        path.display()
    );
}
