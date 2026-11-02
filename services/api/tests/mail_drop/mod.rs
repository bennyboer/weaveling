use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

pub fn code_dropped_for(drop: &Path, address: &str) -> String {
    fs::read_dir(drop)
        .expect("the drop should have been made")
        .map(|entry| {
            serde_json::from_slice::<Value>(
                &fs::read(entry.expect("an entry").path()).expect("readable"),
            )
            .expect("a mail as JSON")
        })
        .filter(|mail| mail["to"] == address)
        .find_map(|mail| {
            mail["body"].as_str().and_then(|body| {
                body.split_whitespace()
                    .find(|word| {
                        word.len() == 6 && word.bytes().all(|digit| digit.is_ascii_digit())
                    })
                    .map(str::to_owned)
            })
        })
        .expect("a code should have been dropped")
}

pub async fn invitation_token_mailed_to(drop: &Path, address: &str) -> Option<String> {
    for _ in 0..1_000 {
        if let Some(token) = invitation_token_dropped_for(drop, address) {
            return Some(token);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    None
}

fn invitation_token_dropped_for(drop: &Path, address: &str) -> Option<String> {
    fs::read_dir(drop)
        .expect("the drop should have been made")
        .map(|entry| {
            serde_json::from_slice::<Value>(
                &fs::read(entry.expect("an entry").path()).expect("readable"),
            )
            .expect("a mail as JSON")
        })
        .filter(|mail| mail["to"] == address)
        .find_map(|mail| {
            mail["body"].as_str().and_then(|body| {
                body.split_once("/invitation#").map(|(_, rest)| {
                    rest.chars()
                        .take_while(char::is_ascii_hexdigit)
                        .collect::<String>()
                })
            })
        })
}
