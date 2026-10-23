# acting

Who acts in a request, and what they may do on the whole instance.

`Acting { agent, operator }` is put into every request behind the gate, beside the `Agent` it holds, by the accounts' gate, which loads the signed-in account for every request anyway: to see that its session still counts, that it is not locked. Whether it is an operator is known at the same moment, at no cost, and always as it now stands, so taking the role away counts on the next request.

**Read from the request, not from another feature.** A feature may not ask accounts who is an operator; invitations, which only an operator may send, read it here as they read the `Agent`.

**Only what holds for the whole instance.** A project's roles, owner, author, reader, differ from project to project, and come from the `Authorizer` asked about that one project, not from here.

In local mode nothing reads it yet, and nothing puts it there; it comes when a route there wants it.
