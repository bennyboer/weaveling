use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::{LoggingMailer, Mail, Mailer, Unsent};

pub struct DroppingMailer {
    into: PathBuf,
    dropped: AtomicU64,
}

impl DroppingMailer {
    pub fn into(directory: impl Into<PathBuf>) -> Self {
        Self {
            into: directory.into(),
            dropped: AtomicU64::new(0),
        }
    }

    fn named(&self) -> String {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or_default();
        let dropped = self.dropped.fetch_add(1, Ordering::Relaxed);

        format!("{since:020}-{dropped:06}.json")
    }
}

#[async_trait]
impl Mailer for DroppingMailer {
    async fn send(&self, mail: Mail) -> Result<(), Unsent> {
        let unsent = |because: String| Unsent {
            to: mail.to.clone(),
            because,
        };
        fs::create_dir_all(&self.into).map_err(|why| unsent(why.to_string()))?;
        let written =
            serde_json::to_vec_pretty(&mail).expect("a mail is plain text and serializes");
        fs::write(self.into.join(self.named()), written).map_err(|why| unsent(why.to_string()))?;

        LoggingMailer.send(mail).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_drop(called: &str) -> PathBuf {
        let drop = std::env::temp_dir().join(format!(
            "weaveling-mail-drop-{called}-{}",
            std::process::id()
        ));
        let _gone = fs::remove_dir_all(&drop);

        drop
    }

    fn a_mail(to: &str, body: &str) -> Mail {
        Mail {
            to: to.to_owned(),
            subject: "Your Weaveling code".to_owned(),
            body: body.to_owned(),
        }
    }

    fn dropped(drop: &PathBuf) -> Vec<Mail> {
        let mut names: Vec<_> = fs::read_dir(drop)
            .expect("the drop should exist")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        names.sort();

        names
            .iter()
            .map(|path| {
                let read: serde_json::Value =
                    serde_json::from_slice(&fs::read(path).expect("readable"))
                        .expect("a mail as JSON");
                a_mail(
                    read["to"].as_str().expect("to"),
                    read["body"].as_str().expect("body"),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn each_mail_is_dropped_as_a_file_of_its_own_in_the_order_sent() {
        let drop = a_drop("order");
        let mailer = DroppingMailer::into(&drop);

        for body in ["first", "second", "third"] {
            mailer
                .send(a_mail("ada@example.com", body))
                .await
                .expect("dropping should succeed");
        }

        assert_eq!(
            dropped(&drop),
            vec![
                a_mail("ada@example.com", "first"),
                a_mail("ada@example.com", "second"),
                a_mail("ada@example.com", "third"),
            ],
            "whoever reads the drop takes the newest mail for an address, so the names must sort as sent"
        );
        let _gone = fs::remove_dir_all(&drop);
    }

    #[tokio::test]
    async fn a_drop_not_there_yet_is_made() {
        let drop = a_drop("made").join("deeper");

        DroppingMailer::into(&drop)
            .send(a_mail("ada@example.com", "123456"))
            .await
            .expect("dropping should succeed");

        assert_eq!(dropped(&drop).len(), 1);
        let _gone = fs::remove_dir_all(drop.parent().expect("a parent"));
    }
}
