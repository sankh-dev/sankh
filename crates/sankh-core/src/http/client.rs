//! HTTP clients: `reqwest` on rustls (ring), one per distinct set of
//! connection options, reused for the whole run.

use crate::parse::spec::{HttpVersion, RequestSpec, Resolve};
use rustls::RootCertStore;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Options that need their own client.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientKey {
    pub insecure: bool,
    pub compressed: bool,
    pub http: HttpVersion,
    pub proxy: Option<String>,
    pub noproxy: Option<String>,
    pub env_proxy: bool,
    pub ca_cert: Option<PathBuf>,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    pub resolve: Vec<Resolve>,
    pub connect_timeout: Option<Duration>,
}

impl ClientKey {
    pub fn for_spec(spec: &RequestSpec, env_proxy: bool) -> ClientKey {
        let o = &spec.options;
        let port = url::Url::parse(&spec.url)
            .ok()
            .and_then(|u| u.port_or_known_default());
        ClientKey {
            insecure: o.insecure,
            compressed: o.compressed,
            http: o.http,
            proxy: o.proxy.clone(),
            noproxy: o.noproxy.clone(),
            env_proxy,
            ca_cert: o.ca_cert.clone(),
            client_cert: o.client_cert.clone(),
            client_key: o.client_key.clone(),
            resolve: o
                .resolve
                .iter()
                .filter(|r| Some(r.port) == port)
                .cloned()
                .collect(),
            connect_timeout: o.connect_timeout,
        }
    }
}

fn provider() -> Arc<CryptoProvider> {
    static P: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    P.get_or_init(|| Arc::new(rustls::crypto::ring::default_provider()))
        .clone()
}

/// The system's trusted roots, or the bundled Mozilla roots when the system
/// has none (slim containers without ca-certificates).
fn system_roots() -> RootCertStore {
    static ROOTS: OnceLock<RootCertStore> = OnceLock::new();
    ROOTS
        .get_or_init(|| {
            let mut store = RootCertStore::empty();
            for cert in rustls_native_certs::load_native_certs().certs {
                let _ = store.add(cert);
            }
            if store.is_empty() {
                store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            }
            store
        })
        .clone()
}

fn read_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let certs: Vec<_> = CertificateDer::pem_file_iter(path)
        .and_then(|it| it.collect::<Result<Vec<_>, _>>())
        .map_err(|e| format!("cannot read certificates from {}: {e}", path.display()))?;
    if certs.is_empty() {
        return Err(format!("{} has no PEM certificates", path.display()));
    }
    Ok(certs)
}

fn tls_config(key: &ClientKey) -> Result<rustls::ClientConfig, String> {
    let provider = provider();
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?;
    let builder = if key.insecure {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerify(
                provider.signature_verification_algorithms,
            )))
    } else {
        let roots = match &key.ca_cert {
            Some(path) => {
                let mut store = RootCertStore::empty();
                let (added, _) = store.add_parsable_certificates(read_certs(path)?);
                if added == 0 {
                    return Err(format!("{} has no usable CA certificates", path.display()));
                }
                store
            }
            None => system_roots(),
        };
        builder.with_root_certificates(roots)
    };
    let mut config = match (&key.client_cert, &key.client_key) {
        (Some(cert), key_path) => {
            let certs = read_certs(cert)?;
            let key_file = key_path.as_deref().unwrap_or(cert);
            let private = PrivateKeyDer::from_pem_file(key_file).map_err(|e| {
                format!(
                    "cannot read a private key from {}: {e}{}",
                    key_file.display(),
                    if key_path.is_none() {
                        " (use --key)"
                    } else {
                        ""
                    }
                )
            })?;
            builder
                .with_client_auth_cert(certs, private)
                .map_err(|e| format!("client certificate: {e}"))?
        }
        (None, Some(_)) => return Err("--key needs --cert".into()),
        (None, None) => builder.with_no_client_auth(),
    };
    config.alpn_protocols = match key.http {
        HttpVersion::Http1 => vec![b"http/1.1".to_vec()],
        HttpVersion::Auto => vec![b"h2".to_vec(), b"http/1.1".to_vec()],
    };
    Ok(config)
}

pub fn build(key: &ClientKey) -> Result<reqwest::Client, String> {
    let mut b = reqwest::Client::builder()
        .tls_backend_preconfigured(tls_config(key)?)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .referer(false)
        .http1_title_case_headers()
        .gzip(key.compressed)
        .brotli(key.compressed)
        .deflate(key.compressed)
        .pool_idle_timeout(Duration::from_secs(30));
    if key.http == HttpVersion::Http1 {
        b = b.http1_only();
    }
    if let Some(t) = key.connect_timeout {
        b = b.connect_timeout(t);
    }
    let noproxy = key.noproxy.as_deref().map(reqwest::NoProxy::from_string);
    match &key.proxy {
        Some(_) if key.noproxy.as_deref() == Some("*") => b = b.no_proxy(),
        Some(p) => {
            let proxy = reqwest::Proxy::all(p).map_err(|e| format!("invalid proxy `{p}`: {e}"))?;
            b = b.proxy(proxy.no_proxy(noproxy.flatten()));
        }
        None if !key.env_proxy || key.noproxy.as_deref() == Some("*") => b = b.no_proxy(),
        None => {
            if let Some(np) = noproxy {
                for (scheme, vars) in [
                    ("http", ["http_proxy", "HTTP_PROXY"]),
                    ("https", ["https_proxy", "HTTPS_PROXY"]),
                ] {
                    let Some(url) = vars
                        .iter()
                        .find_map(|v| std::env::var(v).ok())
                        .filter(|v| !v.is_empty())
                    else {
                        continue;
                    };
                    let proxy = match scheme {
                        "http" => reqwest::Proxy::http(&url),
                        _ => reqwest::Proxy::https(&url),
                    }
                    .map_err(|e| format!("invalid proxy `{url}`: {e}"))?;
                    b = b.proxy(proxy.no_proxy(np.clone()));
                }
            }
        }
    }
    for r in &key.resolve {
        let addrs: Vec<std::net::SocketAddr> = r
            .addrs
            .iter()
            .map(|ip| std::net::SocketAddr::new(*ip, r.port))
            .collect();
        b = b.resolve_to_addrs(&r.host, &addrs);
    }
    b.build()
        .map_err(|e| format!("cannot set up the HTTP client: {e}"))
}

/// `-k`: accepts any certificate, still checking handshake signatures.
#[derive(Debug)]
struct NoVerify(WebPkiSupportedAlgorithms);

impl ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.supported_schemes()
    }
}
