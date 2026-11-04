use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::accounts::service as accounts;
use crate::accounts::service::Answer;
use crate::act::{Act, Here};
use crate::asking::Asking;
use crate::http::ApiError;
use crate::invitations::service;
use crate::invitations::to_you::InvitationToYou;
use crate::live::Subscription;
use crate::notifications::notice::Notice;
use crate::quiet_note::QuietNote;
use crate::speech;
use crate::speech::Speech;

#[derive(Clone, Copy)]
pub struct Invited {
    mine: RwSignal<Vec<InvitationToYou>>,
    answering: Act<(InvitationToYou, Answer)>,
    refused: RwSignal<Option<ApiError>>,
}

impl Invited {
    pub fn following() -> Self {
        let mine = RwSignal::new(Vec::<InvitationToYou>::new());
        let asking = Asking::new();
        let listing = Action::new_local(move |_: &()| {
            let asked = asking.next();

            async move {
                let answer = service::mine().await;
                if !asking.is_latest(asked) {
                    return;
                }
                if let Ok(listed) = answer {
                    _ = mine.try_set(listed);
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

        let refused = RwSignal::new(None);
        let (note, speech) = (expect_context::<QuietNote>(), expect_context::<Speech>());
        let answering = Act::new(
            move |(invitation, given): (InvitationToYou, Answer), _: Here| {
                refused.set(None);

                async move {
                    match accounts::answer_invitation(&invitation.id, given).await {
                        Ok(()) => {
                            _ = mine
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
                        Err(failed) => _ = refused.try_set(Some(failed)),
                    }
                }
            },
        );

        Self {
            mine,
            answering,
            refused,
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
    let accepting = (invitation.clone(), Answer::Accept);
    let declining = (invitation, Answer::Decline);
    let (accepted, declined) = (accepting.clone(), declining.clone());

    html::li().child((
        html::p().class("project").child(named),
        html::p().class("from").child(from),
        html::div().class("answers").child((
            html::button()
                .r#type("button")
                .class("primary")
                .attr("aria-busy", move || invited.answering.busy_on(&accepted))
                .on(ev::click, move |_| {
                    invited.answering.start(accepting.clone())
                })
                .child(speech::said("notice-invitation-join")),
            html::button()
                .r#type("button")
                .attr("aria-busy", move || invited.answering.busy_on(&declined))
                .on(ev::click, move |_| {
                    invited.answering.start(declining.clone())
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
