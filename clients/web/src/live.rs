use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(module = "/js/editor.bundle.js")]
extern "C" {
    type LiveFollowing;

    #[wasm_bindgen(constructor)]
    fn new(path: &str, heard: &Closure<dyn Fn(f64)>, opened: &Closure<dyn Fn()>) -> LiveFollowing;

    #[wasm_bindgen(method)]
    fn close(this: &LiveFollowing);
}

pub struct Following {
    connection: LiveFollowing,
    _heard: Closure<dyn Fn(f64)>,
    _opened: Closure<dyn Fn()>,
}

impl Following {
    pub fn start(path: &str, heard: impl Fn(u64) + 'static, opened: impl Fn() + 'static) -> Self {
        let heard = Closure::<dyn Fn(f64)>::new(move |version: f64| heard(version as u64));
        let opened = Closure::<dyn Fn()>::new(opened);
        let connection = LiveFollowing::new(path, &heard, &opened);

        Self {
            connection,
            _heard: heard,
            _opened: opened,
        }
    }
}

impl Drop for Following {
    fn drop(&mut self) {
        self.connection.close();
    }
}
