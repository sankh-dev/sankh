//! HTTPS against a local server with a self-signed certificate.

mod common;

use common::{build, respond, send, vars};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sankh_core::http::{Engine, SendError};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;

struct Server {
    port: u16,
    cert_pem: String,
}

fn server() -> Server {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert = CertificateDer::from(ck.cert.der().to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(ck.signing_key.serialize_der()));
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert], key)
    .unwrap();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for tcp in listener.incoming().flatten() {
            let config = config.clone();
            std::thread::spawn(move || {
                let conn = rustls::ServerConnection::new(config).unwrap();
                let mut tls = rustls::StreamOwned::new(conn, tcp);
                let mut reader = BufReader::new(&mut tls);
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => return,
                        Ok(_) if line == "\r\n" => break,
                        Ok(_) => {}
                    }
                }
                let _ = tls.write_all(&respond(200, &[], b"secure"));
                let _ = tls.flush();
                tls.conn.send_close_notify();
                let _ = tls.flush();
            });
        }
    });
    Server {
        port,
        cert_pem: ck.cert.pem(),
    }
}

fn cmd(srv: &Server, flags: &str) -> String {
    let p = srv.port;
    format!("curl -sS {flags} --resolve localhost:{p}:127.0.0.1 https://localhost:{p}/")
}

#[test]
fn untrusted_certificates_fail_with_a_hint() {
    let srv = server();
    let root = tempfile::tempdir().unwrap();
    let spec = build(&cmd(&srv, ""), &vars(&[]), root.path());
    match send(&mut Engine::default(), &spec) {
        Err(SendError::Transport(m)) => {
            assert!(
                m.starts_with(&format!("TLS error with localhost:{}", srv.port)),
                "{m}"
            );
            assert!(m.contains("--cacert") && m.contains("-k"), "{m}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn insecure_skips_verification() {
    let srv = server();
    let root = tempfile::tempdir().unwrap();
    let spec = build(&cmd(&srv, "-k"), &vars(&[]), root.path());
    let resp = send(&mut Engine::default(), &spec).unwrap();
    assert_eq!((resp.status, resp.body.as_slice()), (200, &b"secure"[..]));
}

#[test]
fn cacert_trusts_a_private_ca() {
    let srv = server();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ca.pem"), &srv.cert_pem).unwrap();
    let spec = build(&cmd(&srv, "--cacert ca.pem"), &vars(&[]), root.path());
    let resp = send(&mut Engine::default(), &spec).unwrap();
    assert_eq!(resp.status, 200);
}
