use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use yrs::encoding::read::{Cursor, Error, Read};
use yrs::encoding::write::Write;

use crate::live_scenes::PeerId;

/// What a removed client's state reads as in an awareness update.
const GONE: &str = "null";

/// One entry of an awareness update: a client of the editor, how often it has spoken, and what it says now.
/// The state is JSON, or `null` when the client is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub client: u64,
    pub clock: u64,
    pub state: String,
}

impl Said {
    fn is_gone(&self) -> bool {
        self.state == GONE
    }

    /// Whether the state names the given account.
    /// A state that is no JSON object, or names another account or none, does not.
    fn names(&self, account: &str) -> bool {
        serde_json::from_str::<Value>(&self.state)
            .ok()
            .and_then(|state| {
                state
                    .get("account")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .is_some_and(|named| named == account)
    }
}

/// Reads an awareness update into its entries.
pub fn decode(update: &[u8]) -> Result<Vec<Said>, Error> {
    let mut cursor = Cursor::new(update);
    let count = cursor.read_var::<u64>()?;
    let mut said = Vec::new();
    for _ in 0..count {
        said.push(Said {
            client: cursor.read_var()?,
            clock: cursor.read_var()?,
            state: cursor.read_string()?.to_owned(),
        });
    }

    Ok(said)
}

/// Writes entries as an awareness update.
pub fn encode(said: &[Said]) -> Vec<u8> {
    let mut out = Vec::new();
    out.write_var(said.len() as u64);
    for entry in said {
        out.write_var(entry.client);
        out.write_var(entry.clock);
        out.write_string(&entry.state);
    }

    out
}

/// Who speaks for which client of a scene's editors, and at which clock.
/// A connection may only speak for the clients it brought in itself, and only as its own account.
/// When it leaves, its clients can be told gone at once, rather than when the others stop waiting for them.
#[derive(Default)]
pub struct Speakers {
    known: Mutex<HashMap<u64, (PeerId, u64)>>,
}

impl Speakers {
    /// Keeps of an awareness update only what the peer may say, and remembers whom it speaks for.
    /// The peer may tell states that name its own account, and remove clients it spoke for before.
    /// It may not speak for a client another peer speaks for.
    /// Without accounts, `account` is none and any state is kept.
    /// Returns the update to pass on, or nothing when none of it may be passed on.
    pub fn admitted(&self, peer: PeerId, account: Option<&str>, update: &[u8]) -> Option<Vec<u8>> {
        let said = decode(update).ok()?;
        let mut known = self.known.lock().expect("speakers lock poisoned");
        let kept: Vec<Said> = said
            .into_iter()
            .filter(|entry| {
                let speaker = known.get(&entry.client).map(|(by, _)| *by);
                if speaker.is_some_and(|by| by != peer) {
                    return false;
                }
                if entry.is_gone() {
                    return speaker == Some(peer);
                }

                account.is_none_or(|account| entry.names(account))
            })
            .collect();
        for entry in &kept {
            if entry.is_gone() {
                known.remove(&entry.client);
            } else {
                known.insert(entry.client, (peer, entry.clock));
            }
        }

        (!kept.is_empty()).then(|| encode(&kept))
    }

    /// Forgets every client the peer spoke for.
    /// Returns an update that tells them gone, or nothing when the peer spoke for none.
    pub fn farewell(&self, peer: PeerId) -> Option<Vec<u8>> {
        let mut known = self.known.lock().expect("speakers lock poisoned");
        let mut gone: Vec<Said> = known
            .iter()
            .filter(|(_, (by, _))| *by == peer)
            .map(|(client, (_, clock))| Said {
                client: *client,
                clock: *clock,
                state: GONE.to_owned(),
            })
            .collect();
        for entry in &gone {
            known.remove(&entry.client);
        }
        gone.sort_by_key(|entry| entry.client);

        (!gone.is_empty()).then(|| encode(&gone))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saying(client: u64, clock: u64, state: &str) -> Said {
        Said {
            client,
            clock,
            state: state.to_owned(),
        }
    }

    fn update(said: &[Said]) -> Vec<u8> {
        encode(said)
    }

    fn passed(kept: Option<Vec<u8>>) -> Vec<Said> {
        kept.map(|update| decode(&update).expect("a kept update is readable"))
            .unwrap_or_default()
    }

    const ADA: &str = r#"{"account":"account_ada"}"#;
    const GRACE: &str = r#"{"account":"account_grace"}"#;

    #[test]
    fn an_update_is_read_back_as_it_was_written() {
        // given: an update of a state and a removal
        let said = vec![saying(7, 3, ADA), saying(9, 1, GONE)];

        // expect: it reads back the same
        assert_eq!(
            decode(&encode(&said)).expect("a written update is readable"),
            said
        );
    }

    #[test]
    fn a_state_naming_the_peer_s_own_account_is_passed_on() {
        // given: the speakers of a scene
        let speakers = Speakers::default();

        // when: a peer signed in as ada tells her state
        let kept = speakers.admitted(1, Some("account_ada"), &update(&[saying(7, 1, ADA)]));

        // then: it is passed on whole
        assert_eq!(passed(kept), vec![saying(7, 1, ADA)]);
    }

    #[test]
    fn a_state_naming_another_account_or_none_is_dropped() {
        // given: the speakers of a scene
        let speakers = Speakers::default();

        // when: a peer signed in as ada tells states naming grace, nobody, and her
        let kept = speakers.admitted(
            1,
            Some("account_ada"),
            &update(&[
                saying(7, 1, GRACE),
                saying(8, 1, r#"{"user":{"name":"Ada"}}"#),
                saying(9, 1, "not json"),
                saying(10, 1, ADA),
            ]),
        );

        // then: only her own state is passed on
        assert_eq!(passed(kept), vec![saying(10, 1, ADA)]);
    }

    #[test]
    fn nothing_is_passed_on_when_nothing_may_be() {
        // given: the speakers of a scene
        let speakers = Speakers::default();

        // expect: an update naming only another account passes nothing
        assert_eq!(
            speakers.admitted(1, Some("account_ada"), &update(&[saying(7, 1, GRACE)])),
            None
        );
        // and: an update that cannot be read passes nothing
        assert_eq!(speakers.admitted(1, Some("account_ada"), &[200, 1]), None);
    }

    #[test]
    fn a_client_another_peer_speaks_for_cannot_be_spoken_for() {
        // given: a peer that speaks for client 7
        let speakers = Speakers::default();
        speakers.admitted(1, Some("account_ada"), &update(&[saying(7, 1, ADA)]));

        // when: another peer of the same account tells a state for client 7, and removes it
        let told = speakers.admitted(2, Some("account_ada"), &update(&[saying(7, 2, ADA)]));
        let removed = speakers.admitted(2, Some("account_ada"), &update(&[saying(7, 3, GONE)]));

        // then: neither is passed on
        assert_eq!(told, None);
        assert_eq!(removed, None);
    }

    #[test]
    fn a_peer_removes_only_the_clients_it_spoke_for() {
        // given: a peer that speaks for client 7
        let speakers = Speakers::default();
        speakers.admitted(1, Some("account_ada"), &update(&[saying(7, 1, ADA)]));

        // when: it removes client 7 and a client nobody spoke for
        let kept = speakers.admitted(
            1,
            Some("account_ada"),
            &update(&[saying(7, 2, GONE), saying(8, 1, GONE)]),
        );

        // then: only its own client's removal is passed on
        assert_eq!(passed(kept), vec![saying(7, 2, GONE)]);
        // and: it no longer speaks for it, so leaving tells nothing
        assert_eq!(speakers.farewell(1), None);
    }

    #[test]
    fn a_peer_leaving_tells_its_clients_gone_at_their_last_clock() {
        // given: two peers, one speaking for clients 7 and 9, the other for 8
        let speakers = Speakers::default();
        speakers.admitted(1, Some("account_ada"), &update(&[saying(7, 4, ADA)]));
        speakers.admitted(1, Some("account_ada"), &update(&[saying(9, 2, ADA)]));
        speakers.admitted(2, Some("account_grace"), &update(&[saying(8, 1, GRACE)]));

        // when: the first leaves
        let gone = speakers.farewell(1);

        // then: its clients are told gone, each at the clock it last spoke at
        assert_eq!(passed(gone), vec![saying(7, 4, GONE), saying(9, 2, GONE)]);
        // and: the other still speaks for its own
        assert_eq!(passed(speakers.farewell(2)), vec![saying(8, 1, GONE)]);
    }

    #[test]
    fn without_accounts_any_state_is_passed_on() {
        // given: the speakers of a scene on an instance without accounts
        let speakers = Speakers::default();

        // when: a peer tells a state naming no account
        let kept = speakers.admitted(1, None, &update(&[saying(7, 1, r#"{"user":{}}"#)]));

        // then: it is passed on
        assert_eq!(passed(kept), vec![saying(7, 1, r#"{"user":{}}"#)]);
    }
}
