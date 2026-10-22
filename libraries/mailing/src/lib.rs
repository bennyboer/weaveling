use std::sync::Mutex;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Error)]
#[error("a mail to {to} could not be sent: {because}")]
pub struct Unsent {
    pub to: String,
    pub because: String,
}

#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, mail: Mail) -> Result<(), Unsent>;
}

pub struct LoggingMailer;

#[async_trait]
impl Mailer for LoggingMailer {
    async fn send(&self, mail: Mail) -> Result<(), Unsent> {
        tracing::info!(to = %mail.to, subject = %mail.subject, "[mail] {}", mail.body);

        Ok(())
    }
}

#[derive(Default)]
pub struct FakeMailbox {
    sent: Mutex<Vec<Mail>>,
}

impl FakeMailbox {
    pub fn sent_to(&self, to: &str) -> Vec<Mail> {
        self.sent
            .lock()
            .expect("the mailbox is not poisoned")
            .iter()
            .filter(|mail| mail.to == to)
            .cloned()
            .collect()
    }
}

#[async_trait]
impl Mailer for FakeMailbox {
    async fn send(&self, mail: Mail) -> Result<(), Unsent> {
        self.sent
            .lock()
            .expect("the mailbox is not poisoned")
            .push(mail);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_mail(to: &str) -> Mail {
        Mail {
            to: to.to_owned(),
            subject: "Your code".to_owned(),
            body: "123456".to_owned(),
        }
    }

    #[tokio::test]
    async fn the_fake_mailbox_keeps_each_mail_for_whom_it_was_sent() {
        let mailbox = FakeMailbox::default();

        mailbox.send(a_mail("ada@example.com")).await.expect("sent");
        mailbox.send(a_mail("bea@example.com")).await.expect("sent");

        assert_eq!(
            mailbox.sent_to("ada@example.com"),
            vec![a_mail("ada@example.com")]
        );
    }

    #[tokio::test]
    async fn the_logging_mailer_never_fails() {
        assert!(LoggingMailer.send(a_mail("ada@example.com")).await.is_ok());
    }
}
