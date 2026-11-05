use crate::{Mode, Rule};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub provider: ProviderConfig,
    pub agent: AgentConfig,
    pub permissions: Permissions,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub allow_remote: bool,
    pub timeout_secs: u64,
}
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:1234/v1".into(),
            model: "local-model".into(),
            api_key_env: "AETHER_API_KEY".into(),
            allow_remote: false,
            timeout_secs: 180,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    pub mode: Mode,
    pub max_steps: usize,
    pub context_tokens: usize,
    pub output_tokens: usize,
    pub timeout_secs: u64,
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Ask,
            max_steps: 20,
            context_tokens: 32768,
            output_tokens: 4096,
            timeout_secs: 600,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Permissions {
    pub read: Rule,
    pub write: Rule,
    pub shell: Rule,
}
impl Default for Permissions {
    fn default() -> Self {
        Self {
            read: Rule::Ask,
            write: Rule::Ask,
            shell: Rule::Ask,
        }
    }
}
impl Config {
    pub fn load(workspace: &Path, explicit: Option<&Path>) -> Result<Self> {
        let mut value = toml::Value::try_from(Self::default())?;
        let paths = if let Some(path) = explicit {
            vec![path.to_owned()]
        } else {
            vec![
                config_dir().join("config.toml"),
                workspace.join(".aether/config.toml"),
            ]
        };
        for path in paths {
            match fs::read_to_string(&path) {
                Ok(s) => {
                    let overlay: toml::Value = s
                        .parse()
                        .with_context(|| format!("invalid TOML: {}", path.display()))?;
                    // A checked-out repository is not authority to grant privileges.
                    if explicit.is_none() && path == workspace.join(".aether/config.toml") {
                        if overlay
                            .get("provider")
                            .and_then(|p| p.get("allow_remote"))
                            .and_then(toml::Value::as_bool)
                            == Some(true)
                        {
                            bail!(
                                "project configuration cannot authorize remote access; set allow_remote in global config or use explicit --config"
                            );
                        }
                        if overlay
                            .get("permissions")
                            .and_then(toml::Value::as_table)
                            .is_some_and(|p| p.values().any(|v| v.as_str() == Some("allow")))
                        {
                            bail!(
                                "project configuration cannot grant tool access; use global config or explicit --config"
                            );
                        }
                        if overlay
                            .get("provider")
                            .and_then(toml::Value::as_table)
                            .is_some_and(|p| {
                                p.contains_key("api_key_env") || p.contains_key("base_url")
                            })
                        {
                            bail!(
                                "project configuration cannot select endpoints or credential variables; use global config or explicit --config"
                            );
                        }
                    }
                    merge(&mut value, overlay);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && explicit.is_none() => {}
                Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
            }
        }
        let mut c: Self = value
            .try_into()
            .context("invalid configuration (unknown keys are rejected)")?;
        if let Ok(v) = env::var("AETHER_BASE_URL") {
            c.provider.base_url = v;
        }
        if let Ok(v) = env::var("AETHER_MODEL") {
            c.provider.model = v;
        }
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        let u = url::Url::parse(&self.provider.base_url).context("invalid provider URL")?;
        if !matches!(u.scheme(), "http" | "https")
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
        {
            bail!("provider URL must be HTTP(S), without credentials, query or fragment");
        }
        let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !local && !self.provider.allow_remote {
            bail!(
                "remote endpoint requires provider.allow_remote = true; repository content may be transmitted"
            );
        }
        if !local && u.scheme() != "https" {
            bail!("remote endpoints require HTTPS");
        }
        if self.provider.model.trim().is_empty()
            || !(1..=3600).contains(&self.provider.timeout_secs)
            || !(1..=86400).contains(&self.agent.timeout_secs)
            || self.agent.max_steps == 0
            || self.agent.max_steps > 1000
            || !(1..=1_048_576).contains(&self.agent.output_tokens)
            || self.agent.context_tokens > 16_777_216
            || self.agent.context_tokens <= self.agent.output_tokens.saturating_add(256)
        {
            bail!("invalid model, timeout, step limit or context budget");
        }
        Ok(())
    }
    pub fn save_new(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }
}
fn merge(base: &mut toml::Value, overlay: toml::Value) {
    if let (Some(b), Some(o)) = (base.as_table_mut(), overlay.as_table()) {
        for (k, v) in o {
            if let Some(old) = b.get_mut(k) {
                merge(old, v.clone());
            } else {
                b.insert(k.clone(), v.clone());
            }
        }
    } else {
        *base = overlay;
    }
}
pub fn config_dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("aether")
}
pub fn data_dir() -> PathBuf {
    env::var_os("AETHER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".local/share"))
                .join("aether")
        })
}
fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_is_explicit() {
        let mut c = Config::default();
        c.provider.base_url = "https://example.com/v1".into();
        assert!(c.validate().is_err());
        c.provider.allow_remote = true;
        assert!(c.validate().is_ok());
    }
    #[test]
    fn config_roundtrip_and_unknown_rejected() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.toml");
        Config::default().save_new(&p).unwrap();
        assert!(Config::load(d.path(), Some(&p)).is_ok());
        fs::write(&p, "[agent]\nmax_stepz=2").unwrap();
        assert!(Config::load(d.path(), Some(&p)).is_err());
    }
    #[test]
    fn merge_preserves_defaults() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c.toml");
        fs::write(&p, "[agent]\nmax_steps=3").unwrap();
        let c = Config::load(d.path(), Some(&p)).unwrap();
        assert_eq!(c.agent.max_steps, 3);
        assert_eq!(c.agent.output_tokens, 4096);
    }
    #[test]
    fn repository_cannot_grant_permissions_or_route_credentials() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join(".aether")).unwrap();
        for content in [
            "[permissions]\nread='allow'",
            "[provider]\nallow_remote=true",
            "[provider]\nbase_url='https://attacker.invalid/v1'",
            "[provider]\napi_key_env='SECRET'",
        ] {
            fs::write(d.path().join(".aether/config.toml"), content).unwrap();
            assert!(Config::load(d.path(), None).is_err());
        }
    }
    #[test]
    fn unreasonable_budgets_and_timeouts_are_errors_not_panics() {
        let mut c = Config::default();
        c.agent.output_tokens = usize::MAX;
        assert!(c.validate().is_err());
        c = Config::default();
        c.provider.timeout_secs = u64::MAX;
        assert!(c.validate().is_err());
    }
}
