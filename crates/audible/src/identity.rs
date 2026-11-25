use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub key: String,
    pub name: String,
    pub locale: String,
    pub domain: Option<&'static str>,
    pub problem: Option<String>,
    pub(crate) adp_token: String,
    pub(crate) private_key: String,
}

pub fn domain_for(locale: &str) -> Option<&'static str> {
    Some(match locale.trim_start_matches("pre-amazon - ") {
        "us" => "com",
        "uk" => "co.uk",
        "germany" => "de",
        "france" => "fr",
        "canada" => "ca",
        "australia" => "com.au",
        "italy" => "it",
        "spain" => "es",
        "india" => "in",
        "japan" => "co.jp",
        "brazil" => "com.br",
        _ => return None,
    })
}

pub fn default_libation_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        candidates.push(home.join("Libation"));
        candidates.push(home.join(".local/share/Libation"));
        candidates.push(home.join("Library/Application Support/Libation"));
    }
    candidates
        .into_iter()
        .map(|d| d.join("AccountsSettings.json"))
        .find(|p| p.is_file())
}

fn secret(tokens: &Value, field: &str) -> std::result::Result<String, String> {
    let node = &tokens[field];
    if node["IsEncrypted"].as_bool() == Some(true) {
        return Err("Libation stores this sign-in encrypted".into());
    }
    match node["Value"].as_str() {
        Some(v) if !v.is_empty() => Ok(v.to_string()),
        _ => Err("the sign-in is incomplete, sign in again in Libation".into()),
    }
}

fn mask(id: &str) -> String {
    let short = |s: &str| {
        let head: String = s.chars().take(2).collect();
        format!("{head}…")
    };
    match id.split_once('@') {
        Some((user, host)) => format!("{}@{}", short(user), host),
        None => short(id),
    }
}

fn stable_key(id: &str, locale: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.bytes().chain(locale.bytes()) {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn parse_libation(text: &str) -> Result<Vec<Account>> {
    let root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let accounts = root["Accounts"]
        .as_array()
        .ok_or_else(|| Error::Account("the file has no accounts".into()))?;
    Ok(accounts
        .iter()
        .map(|a| {
            let tokens = &a["IdentityTokens"];
            let id = a["AccountId"].as_str().unwrap_or_default();
            let locale = tokens["LocaleName"].as_str().unwrap_or_default().to_string();
            let domain = domain_for(&locale);
            let adp = secret(tokens, "AdpToken");
            let key = secret(tokens, "PrivateKey");
            let problem = if domain.is_none() {
                Some(format!("unknown marketplace: {locale}"))
            } else {
                adp.as_ref().err().or(key.as_ref().err()).cloned()
            };
            let nickname = a["AccountName"].as_str().unwrap_or_default();
            Account {
                key: stable_key(id, &locale),
                name: if nickname.is_empty() || nickname.contains('@') {
                    mask(id)
                } else {
                    nickname.to_string()
                },
                locale,
                domain,
                problem,
                adp_token: adp.unwrap_or_default(),
                private_key: key.unwrap_or_default(),
            }
        })
        .collect())
}

pub fn load_libation(path: &Path) -> Result<Vec<Account>> {
    parse_libation(&std::fs::read_to_string(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accounts_and_flags_unusable_ones() {
        let json = r#"{"Accounts":[
          {"AccountId":"someone@example.com","AccountName":"someone@example.com - us",
           "IdentityTokens":{"LocaleName":"us","AdpToken":{"Value":"{enc:x}"},"PrivateKey":{"Value":"PEM"}}},
          {"AccountId":"other@example.com","AccountName":"Family",
           "IdentityTokens":{"LocaleName":"germany","AdpToken":{"Value":"x","IsEncrypted":true},"PrivateKey":{"Value":"PEM"}}},
          {"AccountId":"x","AccountName":"","IdentityTokens":{"LocaleName":"mars","AdpToken":{"Value":"a"},"PrivateKey":{"Value":"b"}}},
          {"AccountId":"y","AccountName":"Empty","IdentityTokens":{"LocaleName":"uk","AdpToken":{"Value":""},"PrivateKey":{"Value":"b"}}}
        ]}"#;
        let a = parse_libation(json).unwrap();
        assert_eq!(a.len(), 4);
        assert_eq!((a[0].name.as_str(), a[0].domain, a[0].problem.as_ref()), ("so…@example.com", Some("com"), None));
        assert_eq!(a[1].name, "Family");
        assert!(a[1].problem.as_ref().unwrap().contains("encrypted"));
        assert!(a[2].problem.as_ref().unwrap().contains("marketplace"));
        assert!(a[3].problem.as_ref().unwrap().contains("sign in again"));
        assert_ne!(a[0].key, a[1].key);
        assert_eq!(a[0].key, parse_libation(json).unwrap()[0].key);
        assert!(parse_libation("{}").is_err());
    }

    #[test]
    fn marketplaces() {
        assert_eq!(domain_for("germany"), Some("de"));
        assert_eq!(domain_for("pre-amazon - uk"), Some("co.uk"));
        assert_eq!(domain_for("nowhere"), None);
    }
}
