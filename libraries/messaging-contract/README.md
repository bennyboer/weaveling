# messaging-contract

What the server tells the client about messages a listener refused.

`RefusalDTO` is one dead letter as `/api/service/refusals` lists it: which listener refused, the routing key, how often it was tried, why in the words of the code, `plainly` in the words a listener gave for an author (its `WHEN_REFUSED`), when it happened, when it was given up and whether someone has acknowledged it.

The client reads the list as the server sends it:

```rust
use gloo_net::http::Request;
use messaging_contract::RefusalDTO;

let response = Request::get("/api/service/refusals").send().await?;
let refused: Vec<RefusalDTO> = response.json().await?;

for refusal in refused.iter().filter(|refusal| refusal.acknowledged_at.is_none()) {
    let said = refusal.plainly.as_deref().unwrap_or(&refusal.why);
    shown(&refusal.listener, said, refusal.may_be_tried_again);
}
```

**A crate of its own so the client can depend on it** without depending on `messaging`, which brings tokio, the stores and everything a server needs to deliver. The same reason every feature has a `contract`: the browser and the server share the shape and nothing else.
