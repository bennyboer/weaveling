use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Setting, Settings, Unsettled, WEAVELING_CONFIG, totp_key};

pub(super) const BESIDE: &str = "weaveling.toml";
const CONFIG_FLAG: &str = "--config";

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Filed {
    server: FiledServer,
    storage: FiledStorage,
    accounts: FiledAccounts,
    mail: FiledMail,
    assets: FiledAssets,
    testing: FiledTesting,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FiledServer {
    listen: Option<toml::Value>,
    public_url: Option<toml::Value>,
    hosts: Option<toml::Value>,
    client_dir: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FiledStorage {
    database_url: Option<toml::Value>,
    data: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FiledAccounts {
    enabled: Option<toml::Value>,
    sign_up: Option<toml::Value>,
    totp_key: Option<toml::Value>,
    totp_key_file: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FiledMail {
    drop: Option<toml::Value>,
    log: Option<toml::Value>,
    anyone_each_hour: Option<toml::Value>,
    invitations_each_hour: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FiledAssets {
    most_each_upload_mb: Option<toml::Value>,
    most_kept_mb: Option<toml::Value>,
    folder: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FiledTesting {
    flaky: Option<toml::Value>,
}

impl Settings {
    pub(super) fn filed(path: &Path) -> Result<Self, Unsettled> {
        let written = fs::read_to_string(path)
            .map_err(|why| Unsettled::Unreadable(path.to_owned(), why.to_string()))?;
        let filed: Filed = toml::from_str(&written)
            .map_err(|why| Unsettled::Unreadable(path.to_owned(), why.to_string()))?;
        let key = |name: &str| format!("{name} in {}", path.display());

        Ok(Self {
            listen: setting(key("server.listen"), filed.server.listen),
            public_url: setting(key("server.public_url"), filed.server.public_url),
            hosts: setting(key("server.hosts"), filed.server.hosts),
            client_dir: setting(key("server.client_dir"), filed.server.client_dir),
            reload_port: None,
            database_url: setting(key("storage.database_url"), filed.storage.database_url),
            data: setting(key("storage.data"), filed.storage.data),
            accounts: setting(key("accounts.enabled"), filed.accounts.enabled),
            sign_up: setting(key("accounts.sign_up"), filed.accounts.sign_up),
            mail_drop: setting(key("mail.drop"), filed.mail.drop),
            mail_log: setting(key("mail.log"), filed.mail.log),
            mail_anyone_each_hour: setting(
                key("mail.anyone_each_hour"),
                filed.mail.anyone_each_hour,
            ),
            mail_invitations_each_hour: setting(
                key("mail.invitations_each_hour"),
                filed.mail.invitations_each_hour,
            ),
            flaky: setting(key("testing.flaky"), filed.testing.flaky),
            assets_most_each_upload_mb: setting(
                key("assets.most_each_upload_mb"),
                filed.assets.most_each_upload_mb,
            ),
            assets_most_kept_mb: setting(key("assets.most_kept_mb"), filed.assets.most_kept_mb),
            assets_folder: setting(key("assets.folder"), filed.assets.folder),
            totp_key: totp_key::secret(
                setting(key("accounts.totp_key"), filed.accounts.totp_key),
                setting(key("accounts.totp_key_file"), filed.accounts.totp_key_file),
            )?,
        })
    }
}

pub(super) fn named(
    arguments: impl IntoIterator<Item = String>,
    variable: &impl Fn(&str) -> Option<String>,
) -> Option<(String, PathBuf)> {
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == CONFIG_FLAG {
            return arguments
                .next()
                .map(|path| (CONFIG_FLAG.to_owned(), PathBuf::from(path)));
        }
        if let Some(path) = argument.strip_prefix("--config=") {
            return Some((CONFIG_FLAG.to_owned(), PathBuf::from(path)));
        }
    }

    variable(WEAVELING_CONFIG)
        .filter(|path| !path.trim().is_empty())
        .map(|path| (WEAVELING_CONFIG.to_owned(), PathBuf::from(path)))
}

fn setting(source: String, value: Option<toml::Value>) -> Option<Setting> {
    let value = match value? {
        toml::Value::String(written) => written,
        toml::Value::Boolean(true) => "on".to_owned(),
        toml::Value::Boolean(false) => "off".to_owned(),
        toml::Value::Array(listed) => listed
            .iter()
            .map(|each| match each {
                toml::Value::String(written) => written.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    };

    (!value.trim().is_empty()).then(|| Setting::new(source, value))
}
