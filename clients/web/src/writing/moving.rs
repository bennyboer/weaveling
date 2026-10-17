use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Element;

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::writing::bringing_back::Landing;
use crate::writing::choosing::Choosing;
use crate::writing::structure::Structure;
use crate::writing::walk::Stop;

const EDGE_OF_THE_WINDOW: f64 = 60.0;
const SCROLLED_A_MOVE: f64 = 18.0;

#[derive(Clone, Copy)]
pub struct Moving {
    choosing: Choosing,
    structure: Structure,
    carried: RwSignal<Option<Vec<SceneId>>>,
    landing: RwSignal<Option<Landing>>,
}

impl Moving {
    pub fn nothing(choosing: Choosing, structure: Structure) -> Self {
        Self {
            choosing,
            structure,
            carried: RwSignal::new(None),
            landing: RwSignal::new(None),
        }
    }

    pub fn is_carrying(self) -> bool {
        self.carried.with_untracked(Option::is_some)
    }

    pub fn carries(self, scene: &SceneId) -> bool {
        self.carried.with(|carried| {
            carried
                .as_ref()
                .is_some_and(|carried| carried.contains(scene))
        })
    }

    pub fn lands_at(self, landing: &Landing) -> bool {
        self.landing.with(|chosen| chosen.as_ref() == Some(landing))
    }

    pub fn pick_up(self, scene: SceneId) {
        let carried = match untrack(|| self.choosing.holds(&Stop::Scene(scene.clone()))) {
            true => self
                .choosing
                .in_reading_order()
                .into_iter()
                .filter_map(|stop| match stop {
                    Stop::Scene(scene) => Some(scene),
                    _ => None,
                })
                .collect(),
            false => vec![scene],
        };

        self.carried.set(Some(carried));
    }

    pub fn carry_to(self, y: f64) {
        if !self.is_carrying() {
            return;
        }
        let nearest = nearest_gap(y);
        if self.landing.with_untracked(|landing| landing != &nearest) {
            self.landing.set(nearest);
        }

        let tall = window()
            .inner_height()
            .ok()
            .and_then(|tall| tall.as_f64())
            .unwrap_or_default();
        if y < EDGE_OF_THE_WINDOW {
            window().scroll_by_with_x_and_y(0.0, -SCROLLED_A_MOVE);
        } else if y > tall - EDGE_OF_THE_WINDOW {
            window().scroll_by_with_x_and_y(0.0, SCROLLED_A_MOVE);
        }
    }

    pub fn put_down(self) {
        let carried = self.carried.get_untracked();
        let landing = self.landing.get_untracked();
        self.put_back();

        let (Some(carried), Some(Landing { section, after })) = (carried, landing) else {
            return;
        };
        if after.as_ref().is_some_and(|after| carried.contains(after)) {
            return;
        }

        self.choosing.let_go();
        let mut behind = after;
        for scene in carried {
            self.structure
                .bring_back(scene.clone(), section.clone(), behind);
            behind = Some(scene);
        }
    }

    pub fn put_back(self) {
        if self.is_carrying() {
            self.carried.set(None);
            self.landing.set(None);
        }
    }
}

fn nearest_gap(y: f64) -> Option<Landing> {
    let gaps = document()
        .query_selector_all(".writing .insertion[data-section]")
        .ok()?;

    (0..gaps.length())
        .filter_map(|nth| gaps.item(nth)?.dyn_into::<Element>().ok())
        .filter_map(|gap| {
            let seen = gap.get_bounding_client_rect();
            let middle = seen.top() + seen.height() / 2.0;
            let section = SectionId::from(gap.get_attribute("data-section")?);
            let after = gap.get_attribute("data-after").map(SceneId::from);

            Some(((middle - y).abs(), Landing { section, after }))
        })
        .min_by(|(near, _), (nearer, _)| near.total_cmp(nearer))
        .map(|(_, landing)| landing)
}
