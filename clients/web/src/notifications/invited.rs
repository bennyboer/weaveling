use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};

use crate::accounts::service as accounts;
use crate::accounts::service::Answer;
use crate::http::ApiError;
use crate::invitations::service;
use crate::invitations::to_you::InvitationToYou;
use crate::live::Following;
use crate::notifications::notice::Notice;
use crate::quiet_note::QuietNote;
use crate::speech;
use crate::speech::Speech;

#[derive(Clone, Copy)]
pub struct Invited {
    mine: RwSignal<Vec<InvitationToYou>>,
    answering: RwSignal<bool>,
    refused: RwSignal<Option<ApiError>>,
    note: QuietNote,
    speech: Speech,
}

impl Invited {
    pub fn following() -> Self {
        let mine = RwSignal::new(Vec::<InvitationToYou>::new());
        let latest = StoredValue::new(0_u64);
        let listing = Action::new_local(move |_: &()| {
            let asked = latest.get_value() + 1;
            latest.set_value(asked);

            async move {
                let answer = service::mine().await;
                if latest.try_get_value() != Some(asked) {
                    return;
                }
                if let Ok(listed) = answer {
                    _ = mine.try_set(listed);
                }
            }
        });
        listing.dispatch(());
        StoredValue::new_local(Following::start(
            "/api/invitations/mine/live",
            move |_| _ = listing.dispatch(()),
            move || _ = listing.dispatch(()),
        ));

        Self {
            mine,
            answering: RwSignal::new(false),
            refused: RwSignal::new(None),
            note: expect_context::<QuietNote>(),
            speech: expect_context::<Speech>(),
        }
    }

    pub fn notices(self) -> Vec<Notice> {
        self.mine
            .get()
            .into_iter()
            .map(Notice::Invitation)
            .collect()
    }

    pub fn refused(self) -> Option<ApiError> {
        self.refused.get()
    }

    fn answer(self, invitation: InvitationToYou, given: Answer) {
        if self.answering.get_untracked() {
            return;
        }
        let (note, speech) = (self.note, self.speech);
        self.answering.set(true);
        self.refused.set(None);
        task::spawn_local(async move {
            match accounts::answer_invitation(&invitation.id, given).await {
                Ok(()) => {
                    _ = self
                        .mine
                        .try_update(|held| held.retain(|kept| kept.id != invitation.id));
                    let mut told = FluentArgs::new();
                    told.set("project", project_named(&invitation, speech));
                    note.tell(speech.say_with(
                        match given {
                            Answer::Accept => "notice-invitation-joined",
                            Answer::Decline => "notice-invitation-declined",
                        },
                        &told,
                    ));
                }
                Err(failed) => _ = self.refused.try_set(Some(failed)),
            }
            _ = self.answering.try_set(false);
        });
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
    let accepting = invitation.clone();
    let declining = invitation;

    html::li().child((
        html::p().class("project").child(named),
        html::p().class("from").child(from),
        html::div().class("answers").child((
            html::button()
                .r#type("button")
                .class("primary")
                .disabled(move || invited.answering.get())
                .on(ev::click, move |_| {
                    invited.answer(accepting.clone(), Answer::Accept)
                })
                .child(speech::said("notice-invitation-join")),
            html::button()
                .r#type("button")
                .disabled(move || invited.answering.get())
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
