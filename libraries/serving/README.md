# serving

The HTTP conventions that every REST adapter shares. So the four features cannot drift apart on them.

- **`demanded(headers)`** reads `If-Match` into an `Option<Version>`.
  - Missing or `*` means "no expectation".
  - A quoted number means "only if it is still at this version".
  - Anything else is `Unreadable` and becomes a `400`.
- **`tag(version)`** writes the matching `ETag`.
- **`Problem::of(&ServiceError<E>, refused)`** maps an event-sourcing failure to a problem.
  - Not found is a `404`.
  - A stale version is a `412`.
  - Anything else unexpected is logged and becomes a `500` that reveals nothing.
  - A domain refusal is worded by the feature itself. `refused` turns it into a problem of the right kind.

A route that renames an idea reads `If-Match`, responds with an `ETag`, and turns a refusal into a `Problem`:

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

It is small on purpose. Optimistic concurrency is only honest if every route handles it the same way. A feature that parsed `If-Match` by hand would be a feature whose conflicts behave differently from the rest.
