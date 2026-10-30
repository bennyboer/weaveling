# serving

The HTTP conventions every REST adapter shares, so that four features cannot drift apart on them.

- **`demanded(headers)`** reads `If-Match` into an `Option<Version>` — absent or `*` means "no expectation", a quoted number means "only if it still stands at this version", anything else is `Unreadable` and becomes a `400`.
- **`tag(version)`** writes the matching `ETag`.
- **`Problem::of(&ServiceError<E>, refused)`** maps an event-sourcing failure onto a problem: not found is `404`, a stale version is `412`, anything else unexpected is logged and becomes a `500` that says nothing, and a domain refusal is the feature's own to word, `refused` turning it into a problem of its kind.

A route that renames an idea reads `If-Match`, answers with an `ETag`, and turns a refusal into a `Problem`:

```rust
use axum::http::header::ETAG;
use serving::Problem;

async fn retitle(
    State(ideas): State<IdeasCore>,
    Extension(agent): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RetitleIdeaRequest>,
) -> Result<Response, Problem> {
    let retitled = ideas
        .retitle(&id, &request.title, serving::demanded(&headers)?, &agent)
        .await
        .map_err(problem_of)?;

    let tag = serving::tag(retitled.version);

    Ok((StatusCode::OK, [(ETAG, tag)], Json(to_dto(&retitled))).into_response())
}

fn problem_of(refusal: IdeasError) -> Problem {
    match refusal {
        IdeasError::Events(events) => {
            Problem::of(&events, |refused| Problem::refused("discarded", refused))
        }
        other => Problem::unexpected(other),
    }
}
```

Small on purpose. Optimistic concurrency is only honest if every route spells it the same way, and a feature that hand-rolled its own `If-Match` parsing would be a feature whose conflicts behave differently from the rest.
