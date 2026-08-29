use crate::event::EventName;
use crate::version::Version;

type Upgrade<E> = Box<dyn Fn(E) -> E + Send + Sync>;

pub struct Patch<E> {
    name: EventName,
    from: Version,
    upgrade: Upgrade<E>,
}

pub struct Patcher<E> {
    patches: Vec<Patch<E>>,
}

impl<E> Patch<E> {
    pub fn from(
        name: EventName,
        version: Version,
        upgrade: impl Fn(E) -> E + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            from: version,
            upgrade: Box::new(upgrade),
        }
    }
}

impl<E> Patcher<E>
where
    E: crate::event::Event,
{
    pub fn holding(mut patches: Vec<Patch<E>>) -> Self {
        patches.sort_by_key(|patch| patch.from);

        Self { patches }
    }

    pub fn patch(&self, mut event: E) -> E {
        for patch in &self.patches {
            if patch.name == event.name() && patch.from == event.version() {
                event = (patch.upgrade)(event);
            }
        }

        event
    }
}
