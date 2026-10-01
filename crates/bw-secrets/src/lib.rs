//! Secrets dans le Gestionnaire d'identifiants de Windows (trousseau sur macOS).
//!
//! Ailleurs il n'y a pas de coffre : `available()` est faux et les appelants
//! gardent la valeur dans config.toml.

/// Préfixe d'une valeur de config qui renvoie à un secret : `secret:<id>`.
pub const PREFIX: &str = "secret:";

#[cfg(any(windows, target_os = "macos"))]
const SERVICE: &str = "BoringWindows";

/// Un coffre existe sur cette plateforme.
pub const fn available() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

/// Valeur réelle : si `value` est `secret:<id>`, le secret ; sinon `value`.
pub fn resolve(value: &str) -> anyhow::Result<String> {
    match value.trim().strip_prefix(PREFIX) {
        Some(id) => get(id)?.ok_or_else(|| anyhow::anyhow!("secret « {id} » introuvable")),
        None => Ok(value.to_owned()),
    }
}

pub fn is_reference(value: &str) -> bool {
    value.trim().starts_with(PREFIX)
}

/// Identifiant du secret d'une référence `secret:<id>`.
pub fn reference_id(value: &str) -> Option<&str> {
    value.trim().strip_prefix(PREFIX)
}

pub fn reference(id: &str) -> String {
    format!("{PREFIX}{id}")
}

#[cfg(any(windows, target_os = "macos"))]
mod imp {
    use super::SERVICE;

    fn entry(id: &str) -> anyhow::Result<keyring::Entry> {
        Ok(keyring::Entry::new(SERVICE, id)?)
    }

    pub fn get(id: &str) -> anyhow::Result<Option<String>> {
        match entry(id)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set(id: &str, value: &str) -> anyhow::Result<()> {
        Ok(entry(id)?.set_password(value)?)
    }

    pub fn delete(id: &str) -> anyhow::Result<()> {
        match entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn get(_: &str) -> anyhow::Result<Option<String>> {
        anyhow::bail!("pas de coffre à secrets sur cette plateforme")
    }

    pub fn set(_: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("pas de coffre à secrets sur cette plateforme")
    }

    pub fn delete(_: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn get(id: &str) -> anyhow::Result<Option<String>> {
    imp::get(id)
}

pub fn set(id: &str, value: &str) -> anyhow::Result<()> {
    imp::set(id, value)
}

/// Supprime le secret ; sans erreur s'il n'existe pas.
pub fn delete(id: &str) -> anyhow::Result<()> {
    imp::delete(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references() {
        assert_eq!(resolve("https://x/y.ics").unwrap(), "https://x/y.ics");
        assert!(is_reference(" secret:abc"));
        assert_eq!(reference_id("secret:abc"), Some("abc"));
        assert_eq!(reference("cal-1"), "secret:cal-1");
        assert_eq!(reference_id("https://x"), None);
    }

    #[cfg(windows)]
    #[test]
    fn round_trip_in_credential_manager() {
        let id = format!("test-{}", std::process::id());
        set(&id, "valeur").unwrap();
        assert_eq!(get(&id).unwrap().as_deref(), Some("valeur"));
        assert_eq!(resolve(&reference(&id)).unwrap(), "valeur");
        delete(&id).unwrap();
        assert_eq!(get(&id).unwrap(), None);
        delete(&id).unwrap();
    }
}
