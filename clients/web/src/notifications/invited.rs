use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};

use crate::accounts::service as accounts;
use crate::accounts::service::Answer;
use crate::asking::Asking;
use crate::http::ApiError;
use crate::invitations::service;
use crate::invitations::to_you::InvitationToYou;
use crate::live::Subscription;
use crate::notifications::notice::Notice;
use crate::quiet_note::QuietNote;
use crate::speech;
use crate::speech::Speech;
use crate::working_copy;
use crate::working_copy::{Versioned, WorkingCopies};

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeldInvitation {
    invitation: InvitationToYou,
    answering: Option<Answer>,
    turn: u64,
}

#[derive(Clone, Copy)]
pub struct Invited {
    mine: WorkingCopies<String, HeldInvitation, Answer>,
    refused: RwSignal<Option<ApiError>>,
}

impl Invited {
    pub fn following() -> Self {
        let turns = StoredValue::new(0);
        let refused = RwSignal::new(None);
        let (note, speech) = (expect_context::<QuietNote>(), expect_context::<Speech>());
        let mine = WorkingCopies::new(
            move |id: String, on: HeldInvitation, given: Answer| async move {
                accounts::answer_invitation(&id, given).await?;
                let mut told = FluentArgs::new();
                told.set("project", project_named(&on.invitation, speech));
                note.tell(speech.say_with(
                    match given {
                        Answer::Accept => "notice-invitation-joined",
                        Answer::Decline => "notice-invitation-declined",
                    },
                    &told,
                ));

                Ok(answered(on, given, next_turn(turns)))
            },
            move |id: String| {
                let turn = next_turn(turns);

                async move {
                    service::mine()
                        .await?
                        .into_iter()
                        .find(|invitation| invitation.id == id)
                        .map(|invitation| held(invitation, turn).1)
                        .ok_or(ApiError::NotFound("invitation"))
                }
            },
            move |failure| _ = refused.try_set(Some(failure)),
        );
        let asking = Asking::new();
        let listing = Action::new_local(move |_: &()| {
            let (asked, turn) = (asking.next(), next_turn(turns));

            async move {
                let answer = service::mine().await;
                if !asking.is_latest(asked) {
                    return;
                }
                if let Ok(listed) = answer {
                    mine.take_in_all(
                        listed
                            .into_iter()
                            .map(|invitation| held(invitation, turn))
                            .collect(),
                    );
                }
            }
        });
        listing.dispatch(());
        StoredValue::new_local(Subscription::start(
            "invitations",
            &[("mine", "true")],
            move |_| _ = listing.dispatch(()),
            move || _ = listing.dispatch(()),
        ));

        Self { mine, refused }
    }

    pub fn notices(self) -> Vec<Notice> {
        self.mine
            .shown()
            .into_iter()
            .map(|held| Notice::Invitation(held.invitation))
            .collect()
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }

    fn answer(self, id: String, given: Answer) {
        let answered_or_gone =
            untrack(|| !self.mine.is_listed(&id) || self.mine.unanswered_on(&id));
        if answered_or_gone {
            return;
        }
        self.refused.set(None);
        let Some(landed) = self.mine.intended(&id, given) else {
            return;
        };
        task::spawn_local(async move {
            if landed.await {
                self.mine.forget(&id);
            }
        });
    }

    fn busy_on(self, id: &String, given: Answer) -> Option<&'static str> {
        self.mine
            .shown_one(id)
            .is_some_and(|held| held.answering == Some(given))
            .then_some("true")
    }
}

impl Versioned for HeldInvitation {
    fn version(&self) -> u64 {
        self.turn
    }
}

impl working_copy::Intent<HeldInvitation> for Answer {
    fn lay_over(&self, on: &mut HeldInvitation) {
        on.answering = Some(*self);
    }

    fn merged_after(&self, _: &Self) -> Option<Self> {
        None
    }
}

fn next_turn(turns: StoredValue<u64>) -> u64 {
    turns
        .try_update_value(|turn| {
            *turn += 1;
            *turn
        })
        .unwrap_or_default()
}

fn held(invitation: InvitationToYou, turn: u64) -> (String, HeldInvitation) {
    (
        invitation.id.clone(),
        HeldInvitation {
            invitation,
            answering: None,
            turn,
        },
    )
}

fn answered(on: HeldInvitation, given: Answer, turn: u64) -> HeldInvitation {
    HeldInvitation {
        answering: Some(given),
        turn,
        ..on
    }
}

pub fn row(invited: Invited, invitation: InvitationToYou) -> impl IntoView {
    let speech = expect_context::<Speech>();
    let named = {
        let invitation = invitation.clone();
        move || project_named(&invitation, speech)
    };
    let from = {
        let invitation = invitation.clone();
        move || {
            let role = speech.say(invitation.role.message());
            match &invitation.invited_by {
                Some(by) => {
                    let mut told = FluentArgs::new();
                    told.set("by", by.clone());
                    format!(
                        "{} · {role}",
                        speech.say_with("notice-invitation-from", &told)
                    )
                }
                None => role,
            }
        }
    };
    let (accepting, declining) = (invitation.id.clone(), invitation.id);
    let (accepted, declined) = (accepting.clone(), declining.clone());

    html::li().child((
        html::p().class("project").child(named),
        html::p().class("from").child(from),
        html::div().class("answers").child((
            html::button()
                .r#type("button")
                .class("primary")
                .attr("aria-busy", move || {
                    invited.busy_on(&accepted, Answer::Accept)
                })
                .on(ev::click, move |_| {
                    invited.answer(accepting.clone(), Answer::Accept)
                })
                .child(speech::said("notice-invitation-join")),
            html::button()
                .r#type("button")
                .attr("aria-busy", move || {
                    invited.busy_on(&declined, Answer::Decline)
                })
                .on(ev::click, move |_| {
                    invited.answer(declining.clone(), Answer::Decline)
                })
                .child(speech::said("notice-invitation-decline")),
        )),
    ))
}

fn project_named(invitation: &InvitationToYou, speech: Speech) -> String {
    invitation
        .project_name
        .clone()
        .unwrap_or_else(|| speech.say("notice-invitation-unnamed-project"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::listed::Role;
    use crate::working_copy::Intent as _;

    fn an_invitation_to_shared() -> HeldInvitation {
        held(
            InvitationToYou {
                id: "invitation_7".to_owned(),
                project_name: Some("Shared".to_owned()),
                role: Role::Author,
                invited_by: Some("Grace".to_owned()),
            },
            1,
        )
        .1
    }

    #[test]
    fn an_answer_on_its_way_marks_the_invitation_with_it() {
        // given: an invitation not yet answered
        let open = an_invitation_to_shared();

        // when: joining it is laid over it
        let mut shown = open.clone();
        Answer::Accept.lay_over(&mut shown);

        // then: it is marked as being joined, the rest as it was
        assert_eq!(shown.answering, Some(Answer::Accept));
        assert_eq!(shown.invitation, open.invitation);
    }

    #[test]
    fn an_answer_landed_stands_on_a_newer_turn() {
        // when: declining an invitation read in turn one is answered in turn two
        let answered = answered(an_invitation_to_shared(), Answer::Decline, 2);

        // then: it is declined, at the newer turn
        assert_eq!(answered.answering, Some(Answer::Decline));
        assert_eq!(answered.version(), 2);
    }

    #[test]
    fn answers_are_never_merged() {
        // expect: a second answer is sent on its own, as each is heard of
        assert_eq!(Answer::Decline.merged_after(&Answer::Accept), None);
        assert_eq!(Answer::Accept.merged_after(&Answer::Accept), None);
    }
}
