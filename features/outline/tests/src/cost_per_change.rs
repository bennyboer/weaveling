use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use axum_test::TestServer;
use clock::{Clock, SystemClock};
use eventsourcing::{Agent, AggregateId, EventStore};
use messaging::{Listener, Message, Publisher, Undelivered};
use outbox::Outbox;
use outline_contract::{AttachRequest, AttachmentDTO};
use outline_core::{
    Attachment, OutlineCatalog, OutlineEvent, OutlineId, OutlineService, SceneLink, SectionTitle,
};
use outline_wiring::{OutlineFeature, Ports};
use test_harness::SqliteFixture;
use wiring::{Context, Feature};

const SIZES: [usize; 3] = [1_000, 2_500, 5_000];
const SAMPLES: usize = 9;
const SPAN: usize = 100;
const CHAPTER: usize = 50;

#[derive(Default)]
struct Overheard(Mutex<Vec<Message>>);

#[async_trait]
impl Publisher for Overheard {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        self.0.lock().expect("never poisoned").push(message);

        Ok(())
    }
}

impl Overheard {
    fn last(&self) -> Message {
        self.0
            .lock()
            .expect("never poisoned")
            .last()
            .cloned()
            .expect("something was published")
    }
}

struct Bench {
    outlines: OutlineService,
    events: Arc<dyn EventStore<OutlineEvent>>,
    catalog: Arc<dyn OutlineCatalog>,
    outbox: Arc<dyn Outbox>,
    heard: Arc<Overheard>,
    indexer: Arc<dyn Listener>,
    server: TestServer,
    outline: OutlineId,
    section: String,
    attached: usize,
}

fn median(mut taken: Vec<Duration>) -> Duration {
    taken.sort();
    taken[taken.len() / 2]
}

fn ms(taken: Duration) -> String {
    format!("{:.1}", taken.as_secs_f64() * 1_000.0)
}

fn telling(heard: &Arc<Overheard>) -> Context {
    Context {
        clock: Arc::new(SystemClock) as Arc<dyn Clock>,
        publisher: heard.clone(),
    }
}

async fn timed<T>(work: impl IntoFuture<Output = T>) -> (T, Duration) {
    let began = Instant::now();
    let done = work.await;

    (done, began.elapsed())
}

impl Bench {
    async fn on(ports: Ports, heard: Arc<Overheard>) -> Self {
        let context = telling(&heard);
        let wired = outline_wiring::wire(&ports, &context);
        let outlines = outline_wiring::service(&ports, &context);
        let indexer = wired
            .listeners
            .iter()
            .find(|listener| listener.named().to_string() == "index-attachments")
            .cloned()
            .expect("the outline indexes its attachments");
        let opened = outlines
            .open("project_bench", &Agent::Anonymous)
            .await
            .expect("an outline opens");
        let section = outlines
            .add(
                &opened.id.to_string(),
                None,
                None,
                SectionTitle::new("Part One").expect("a plain title"),
                None,
                &Agent::Anonymous,
            )
            .await
            .expect("a section is added")
            .section;

        Self {
            server: TestServer::new(wired.routes),
            outlines,
            events: ports.events.clone(),
            catalog: ports.catalog.clone(),
            outbox: ports.outbox.clone(),
            heard,
            indexer,
            outline: opened.id,
            section: section.to_string(),
            attached: 0,
        }
    }

    async fn next_scene(&mut self) -> String {
        if self.attached > 0 && self.attached.is_multiple_of(CHAPTER) {
            self.section = self
                .outlines
                .add(
                    &self.outline.to_string(),
                    None,
                    Some(self.section.parse().expect("a section id")),
                    SectionTitle::new("Chapter").expect("a plain title"),
                    None,
                    &Agent::Anonymous,
                )
                .await
                .expect("a chapter is added")
                .section
                .to_string();
        }
        self.attached += 1;

        format!("scene_bench{:010}", self.attached)
    }

    async fn attach_one(&mut self) -> Duration {
        let scene = self.next_scene().await;
        let (attached, taken) = timed(self.outlines.attach(
            &self.outline.to_string(),
            Attachment::Scene(SceneLink::from(scene)),
            self.section.parse().expect("a section id"),
            None,
            None,
            &Agent::Anonymous,
        ))
        .await;
        attached.expect("attaching should succeed");

        taken
    }

    async fn grow_to(&mut self, size: usize) {
        while self.attached < size {
            self.attach_one().await;
        }
    }

    async fn measured(&mut self, size: usize) -> String {
        let key = AggregateId::from(&self.outline);
        let mut loading = Vec::new();
        let mut snapshot_reading = Vec::new();
        for _ in 0..SAMPLES {
            loading.push(timed(self.outlines.get(&self.outline.to_string())).await.1);
            snapshot_reading.push(
                timed(self.events.latest_snapshot(&key, outline_core::KIND))
                    .await
                    .1,
            );
        }

        let mut attaching = Vec::new();
        for _ in 0..SPAN {
            attaching.push(self.attach_one().await);
        }
        let slowest = attaching.iter().max().copied().unwrap_or_default();

        let held = self
            .outlines
            .get(&self.outline.to_string())
            .await
            .expect("the outline loads")
            .state
            .attachments();
        let mut rewriting = Vec::new();
        for _ in 0..SAMPLES {
            let (rewritten, taken) = timed(self.catalog.holds(self.outline, &held)).await;
            rewritten.expect("the index is rewritten");
            rewriting.push(taken);
        }

        let mut answering = Vec::new();
        let mut answered_bytes = 0;
        let mut indexing = Vec::new();
        for _ in 0..SAMPLES {
            let scene = self.next_scene().await;
            let (answer, taken) = timed(
                self.server
                    .post(&format!("/outlines/{}/attachments", self.outline))
                    .json(&AttachRequest {
                        attachment: AttachmentDTO::Scene { id: scene },
                        section: self.section.clone(),
                        after: None,
                    }),
            )
            .await;
            answer.assert_status_ok();
            answered_bytes = answer.as_bytes().len();
            answering.push(taken);

            self.outbox.deliver(1_000).await.expect("delivering works");
            let attached = self.heard.last();
            let (indexed, taken) = timed(self.indexer.handle(&attached)).await;
            indexed.expect("indexing should succeed");
            indexing.push(taken);
        }

        format!(
            "{size:>6} scenes | load {:>7} ms (snapshot read {:>7}) | attach {:>7} ms, slowest of {SPAN} {:>7} | REST attach {:>7} ms, answer {:>5} KB | index listener {:>7} ms, of it writing the index {:>7} ms",
            ms(median(loading)),
            ms(median(snapshot_reading)),
            ms(median(attaching)),
            ms(slowest),
            ms(median(answering)),
            answered_bytes / 1_024,
            ms(median(indexing)),
            ms(median(rewriting)),
        )
    }

    async fn report(mut self, backend: &str) {
        println!("{backend}");
        for size in SIZES {
            self.grow_to(size).await;
            println!("{}", self.measured(size).await);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement, run on purpose: cargo test -p weaveling-outline-tests --release cost_per_change -- --ignored --nocapture"]
async fn cost_per_change() {
    let heard = Arc::new(Overheard::default());
    let ports = OutlineFeature::in_memory(&telling(&heard));
    Bench::on(ports, heard).await.report("memory").await;

    let fixture = SqliteFixture::setup();
    let pool = fixture.create_database("outline").await;
    for schema in OutlineFeature::sqlite_schema() {
        schema.run(&pool).await.expect("the schema lays down");
    }
    let heard = Arc::new(Overheard::default());
    let ports =
        OutlineFeature::on_sqlite(pool, &telling(&heard)).expect("the outline wires on SQLite");
    Bench::on(ports, heard).await.report("sqlite").await;
    fixture.cleanup().await;
}
