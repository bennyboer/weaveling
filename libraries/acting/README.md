# acting

Who acts in a request, and what they may do on the whole instance.

`Acting { agent, operator }` is added to every request that passes the gate. It sits next to the `Agent` it contains. The accounts gate adds it. That gate loads the signed-in account on every request anyway, to check that its session is still valid and that the account is not locked. So whether the account is an operator is known at the same moment, at no extra cost. It is also always up to date, so when the operator role is taken away, this counts from the next request on.

A route takes it as `Extension(acting): Extension<Acting>` and passes it on. The invitations feature asks it whether the account is an operator:

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

**Read it from the request, not from another feature.** A feature may not ask the accounts feature who is an operator. Only an operator may send invitations to the instance. So the invitations feature reads the operator flag here, the same way it reads the `Agent`.

**It holds only what applies to the whole instance.** A project's roles (owner, author, reader) differ from project to project. They come from the `Authorizer`, asked about that one project, not from here.

In local mode nothing reads it yet, and nothing adds it to the request. It will be added once a route in local mode needs it.
