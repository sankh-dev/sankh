//! On Windows, downloads the official curl build and the Mozilla CA bundle,
//! checks their sha256, and emits `bundled_curl.rs` for `include_bytes!`.
//!
//! Offline: set `SANKH_CURL_DIST` to the zip and `SANKH_CURL_CA` to the
//! `cacert.pem` file. Both checksums still have to match.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const CURL_VERSION: &str = "8.23.0_5";
const CURL_URL: &str = "https://curl.se/windows/dl-8.23.0_5/curl-8.23.0_5-win64-mingw.zip";
const CURL_SHA256: &str = "b0af3896433242213f6d1388a104115e6dfdd3b16727e5b00c2ab0f48fbc904a";
/// Mozilla CA bundle as of Tue Oct 20 13:32:40 2026 GMT.
const CA_URL: &str = "https://curl.se/ca/cacert.pem";
const CA_SHA256: &str = "796f1389bfea13c0ed78f663b765baaff54a508e5483786bd6ece73c5e3e3cbe";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SANKH_CURL_DIST");
    println!("cargo:rerun-if-env-changed=SANKH_CURL_CA");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let curl_exe = out_dir.join("curl.exe");
    let ca_file = out_dir.join("curl-ca-bundle.crt");
    let stamp = out_dir.join("curl-bundle.stamp");
    let stamp_body = format!("{CURL_VERSION}\n{CURL_SHA256}\n{CA_SHA256}\n");
    if stamp.is_file()
        && std::fs::read_to_string(&stamp).ok().as_deref() == Some(stamp_body.as_str())
        && curl_exe.is_file()
        && ca_file.is_file()
    {
        write_bundle_rs(&out_dir, &curl_exe, &ca_file);
        return;
    }

    let zip_path = out_dir.join("curl-dist.zip");
    match std::env::var("SANKH_CURL_DIST") {
        Ok(local) => {
            std::fs::copy(&local, &zip_path).unwrap_or_else(|e| {
                panic!(
                    "cannot copy SANKH_CURL_DIST ({local}) to {}: {e}",
                    zip_path.display()
                )
            });
        }
        Err(_) => download(CURL_URL, &zip_path, "SANKH_CURL_DIST"),
    }
    verify_sha256(&zip_path, CURL_SHA256);
    extract_curl_exe(&zip_path, &curl_exe);

    match std::env::var("SANKH_CURL_CA") {
        Ok(local) => {
            std::fs::copy(&local, &ca_file).unwrap_or_else(|e| {
                panic!(
                    "cannot copy SANKH_CURL_CA ({local}) to {}: {e}",
                    ca_file.display()
                )
            });
        }
        Err(_) => download(CA_URL, &ca_file, "SANKH_CURL_CA"),
    }
    verify_sha256(&ca_file, CA_SHA256);

    std::fs::write(&stamp, stamp_body).expect("write curl bundle stamp");
    write_bundle_rs(&out_dir, &curl_exe, &ca_file);
}

fn download(url: &str, dest: &Path, env_var: &str) {
    let response = ureq::get(url).call().unwrap_or_else(|e| {
        panic!(
            "failed to download {url}: {e}\nSet {env_var} to a local file with sha256 checked by build.rs."
        )
    });
    let status = response.status();
    if status != 200 {
        panic!("failed to download {url}: HTTP {status}. Set {env_var} to a local file.");
    }
    let bytes = response
        .into_body()
        .read_to_vec()
        .unwrap_or_else(|e| panic!("failed to read {url}: {e}"));
    std::fs::write(dest, bytes).unwrap_or_else(|e| panic!("cannot write {}: {e}", dest.display()));
}

fn verify_sha256(path: &Path, expected: &str) {
    let mut file =
        File::open(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        panic!(
            "checksum mismatch for {}: expected {expected}, got {actual}",
            path.display()
        );
    }
}

fn extract_curl_exe(zip_path: &Path, dest: &Path) {
    let file =
        File::open(zip_path).unwrap_or_else(|e| panic!("cannot open {}: {e}", zip_path.display()));
    let mut archive = zip::ZipArchive::new(file)
        .unwrap_or_else(|e| panic!("cannot read zip {}: {e}", zip_path.display()));
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .unwrap_or_else(|e| panic!("zip entry {i}: {e}"));
        let name = entry.name().replace('\\', "/");
        if !name.ends_with("/bin/curl.exe") || name.contains("..") {
            continue;
        }
        let mut out =
            File::create(dest).unwrap_or_else(|e| panic!("cannot create {}: {e}", dest.display()));
        std::io::copy(&mut entry, &mut out)
            .unwrap_or_else(|e| panic!("cannot extract curl.exe: {e}"));
        return;
    }
    panic!("zip {} does not contain bin/curl.exe", zip_path.display());
}

fn write_bundle_rs(out_dir: &Path, curl_exe: &Path, ca_file: &Path) {
    let curl = rust_path(curl_exe);
    let ca = rust_path(ca_file);
    let rs = format!(
        r#"pub const VERSION: &str = "{CURL_VERSION}";
pub static FILES: &[(&str, &[u8])] = &[
    ("curl.exe", include_bytes!("{curl}")),
    ("curl-ca-bundle.crt", include_bytes!("{ca}")),
];
"#
    );
    let path = out_dir.join("bundled_curl.rs");
    let mut file =
        File::create(&path).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    file.write_all(rs.as_bytes())
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

fn rust_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}
