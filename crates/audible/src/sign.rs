use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};

use crate::{Error, Result};

pub struct Signer {
    key: RsaKeyPair,
    adp_token: String,
}

fn pem_body(pem: &str) -> Result<(bool, Vec<u8>)> {
    let pkcs8 = pem.contains("BEGIN PRIVATE KEY");
    let b64: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    let der = STANDARD
        .decode(b64)
        .map_err(|_| Error::Account("the device key is not valid PEM".into()))?;
    Ok((pkcs8, der))
}

impl Signer {
    pub fn new(private_key_pem: &str, adp_token: &str) -> Result<Self> {
        let (pkcs8, der) = pem_body(private_key_pem)?;
        let key = if pkcs8 {
            RsaKeyPair::from_pkcs8(&der)
        } else {
            RsaKeyPair::from_der(&der)
        }
        .map_err(|_| Error::Account("the device key could not be loaded".into()))?;
        Ok(Self {
            key,
            adp_token: adp_token.to_string(),
        })
    }

    pub fn sign(&self, method: &str, path_and_query: &str, body: &str, timestamp: &str) -> Result<String> {
        let message = format!(
            "{method}\n{path_and_query}\n{timestamp}\n{body}\n{}",
            self.adp_token
        );
        let mut sig = vec![0u8; self.key.public().modulus_len()];
        self.key
            .sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), message.as_bytes(), &mut sig)
            .map_err(|_| Error::Account("signing the request failed".into()))?;
        Ok(format!("{}:{timestamp}", STANDARD.encode(sig)))
    }

    pub fn adp_token(&self) -> &str {
        &self.adp_token
    }
}

pub fn signature_headers(
    signer: &Signer,
    method: &str,
    path_and_query: &str,
    body: &str,
) -> Result<[(&'static str, String); 3]> {
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    Ok([
        ("x-adp-token", signer.adp_token().to_string()),
        ("x-adp-alg", "SHA256withRSA:1.0".to_string()),
        (
            "x-adp-signature",
            signer.sign(method, path_and_query, body, &timestamp)?,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> String {
        format!(
            "-----BEGIN RSA {0}-----\n{1}\n-----END RSA {0}-----\n",
            "PRIVATE KEY",
            include_str!("../tests/test_key.txt").trim()
        )
    }

    #[test]
    fn signs_deterministically_and_rejects_bad_keys() {
        let signer = Signer::new(&key(), "{enc:token}").unwrap();
        let a = signer.sign("GET", "/1.0/library?page=1", "", "2026-01-01T00:00:00.000Z").unwrap();
        let b = signer.sign("GET", "/1.0/library?page=1", "", "2026-01-01T00:00:00.000Z").unwrap();
        let c = signer.sign("GET", "/1.0/library?page=2", "", "2026-01-01T00:00:00.000Z").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        let (sig, ts) = a.rsplit_once(':').map(|_| a.split_at(a.find(':').unwrap())).unwrap();
        assert_eq!(STANDARD.decode(sig).unwrap().len(), 256);
        assert_eq!(ts, ":2026-01-01T00:00:00.000Z");
        assert!(Signer::new("not a key", "x").is_err());
        let headers = signature_headers(&signer, "GET", "/x", "").unwrap();
        assert_eq!(headers[1].1, "SHA256withRSA:1.0");
    }
}
