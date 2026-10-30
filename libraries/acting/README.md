# acting

Who acts in a request, and what they may do on the whole instance.

`Acting { agent, operator }` is put into every request behind the gate, beside the `Agent` it holds, by the accounts' gate, which loads the signed-in account for every request anyway: to see that its session still counts, that it is not locked. Whether it is an operator is known at the same moment, at no cost, and always as it now stands, so taking the role away counts on the next request.

A route takes it as `Extension(acting): Extension<Acting>` and hands it on; the invitations ask it whether it is an operator:

```rust
use acting::Acting;

pub async fn invite_to_instance(
    &self,
    by: &Acting,
    to: &str,
    origin: &str,
) -> Result<InvitationSummary, InvitationsError> {
    if !by.operator {
        return Err(InvitationsError::NotAnOperator);
    }

    self.service
        .issue(to, origin, Scope::Instance, true, &by.agent)
        .await
}
```

**Read from the request, not from another feature.** A feature may not ask accounts who is an operator; invitations, which only an operator may send, read it here as they read the `Agent`.

**Only what holds for the whole instance.** A project's roles, owner, author, reader, differ from project to project, and come from the `Authorizer` asked about that one project, not from here.

**And when the session ends, `SessionEnd`.** The same gate puts it into the request beside `Acting`: `reached()` returns once the session is over, signed out here or elsewhere, its account locked, its password reset, or its last moment come; accounts make it, behind the `Ending` trait, so whatever holds a request open, the one live socket and the editing socket, closes on it without knowing accounts. Where nothing puts one there, `SessionEnd::never()` never ends.

In local mode nothing reads it yet, and nothing puts it there; it comes when a route there wants it.
