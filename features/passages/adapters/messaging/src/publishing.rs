use messaging::{Message, RoutingKey};
use passages_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, PassageDeletedDTO};
use passages_core::PassageChange;
use serde_json::Value;
use time::OffsetDateTime;

pub fn message_for(change: &PassageChange, at: OffsetDateTime) -> Message {
    let (routing, payload) = match change {
        PassageChange::IdeaLinked { passage, idea } => (
            IDEA_LINKED,
            to_payload(&IdeaLinkDTO {
                passage: passage.to_string(),
                idea: idea.to_string(),
            }),
        ),
        PassageChange::IdeaUnlinked { passage, idea } => (
            IDEA_UNLINKED,
            to_payload(&IdeaLinkDTO {
                passage: passage.to_string(),
                idea: idea.to_string(),
            }),
        ),
        PassageChange::Deleted { passage } => (
            DELETED,
            to_payload(&PassageDeletedDTO {
                passage: passage.to_string(),
            }),
        ),
    };

    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key holds no wildcards"),
        payload,
        at,
    )
}

fn to_payload(dto: &impl serde::Serialize) -> Value {
    serde_json::to_value(dto).expect("an announcement is plain data and cannot fail to serialize")
}

#[cfg(test)]
mod tests {
    use passages_core::{IdeaLink, PassageId};
    use serde_json::json;
    use time::Duration;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_passage() -> PassageId {
        PassageId::generate(at(1_000))
    }

    #[test]
    fn a_link_is_announced_with_both_ends() {
        let passage = a_passage();

        let told = message_for(
            &PassageChange::IdeaLinked {
                passage,
                idea: IdeaLink::from("idea_1"),
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), IDEA_LINKED);
        assert_eq!(
            told.payload,
            json!({ "passage": passage.to_string(), "idea": "idea_1" }),
            "a listener cannot ask passages which idea it was, so both ends travel in the message"
        );
        assert_eq!(told.occurred_at, at(2_000));
    }

    #[test]
    fn an_unlink_is_announced_under_its_own_key() {
        let told = message_for(
            &PassageChange::IdeaUnlinked {
                passage: a_passage(),
                idea: IdeaLink::from("idea_1"),
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), IDEA_UNLINKED);
    }

    #[test]
    fn a_deletion_names_the_passage() {
        let passage = a_passage();

        let told = message_for(&PassageChange::Deleted { passage }, at(2_000));

        assert_eq!(told.routing.to_string(), DELETED);
        assert_eq!(told.payload, json!({ "passage": passage.to_string() }));
    }

    #[test]
    fn every_announcement_reads_back_as_its_contract_shape() {
        let passage = a_passage();

        let linked: IdeaLinkDTO = serde_json::from_value(
            message_for(
                &PassageChange::IdeaLinked {
                    passage,
                    idea: IdeaLink::from("idea_1"),
                },
                at(2_000),
            )
            .payload,
        )
        .expect("a link reads back as the contract says");
        let deleted: PassageDeletedDTO = serde_json::from_value(
            message_for(&PassageChange::Deleted { passage }, at(2_000)).payload,
        )
        .expect("a deletion reads back as the contract says");

        assert_eq!(linked.idea, "idea_1");
        assert_eq!(deleted.passage, passage.to_string());
    }
}
