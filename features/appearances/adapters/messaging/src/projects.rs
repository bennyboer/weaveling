use messaging::Message;
use projects_contract::context::PROJECT;

pub(crate) fn project_of(message: &Message) -> Option<String> {
    eventpublishing::context_in(message)
        .get(PROJECT)
        .map(str::to_owned)
}
