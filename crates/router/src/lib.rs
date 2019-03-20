//! A router inspired by ReasonML's
#![deny(missing_docs, missing_debug_implementations)]


use wasm_bindgen::UnwrapThrowExt;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};


/// Ideally, this will be moved into a separate crate along with other utility functions.
mod util {
    use wasm_bindgen::UnwrapThrowExt;
    /// Convenience function to avoid repeating expect logic.
    pub fn window() -> web_sys::Window {
        web_sys::window().expect_throw("Can't find the global Window")
    }

    /// Convenience function to access the web_sys DOM document.
    pub fn document() -> web_sys::Document {
        window().document().expect_throw("Can't find document")
    }
}

/// Contains all information used in pushing and handling routes.
/// Based on React-Reason's router:
/// https://github.com/reasonml/reason-react/blob/master/docs/router.md
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Url {
    /// Path
    pub path: Vec<String>,
    /// Hash
    pub hash: Option<String>,
    ///Search
    pub search: Option<String>,
    /// (Unimplemented in browsers)
    pub title: Option<String>,
}

impl Url {
    /// Helper that ignores hash, search and title, and converts path to Strings.
    /// https://developer.mozilla.org/en-US/docs/Web/API/History_API
    pub fn new<T: ToString>(path: Vec<T>) -> Self {
        let result = Self {
            path: path.into_iter().map(|p| p.to_string()).collect(),
            hash: None,
            search: None,
            title: None,
        };
        clean_url(result)
    }

    /// Builder-pattern method for defining hash.
    /// https://developer.mozilla.org/en-US/docs/Web/API/HTMLHyperlinkElementUtils/hash
    pub fn hash(mut self, hash: &str) -> Self {
        self.hash = Some(hash.into());
        self
    }

    /// Builder-pattern method for defining search.
    /// https://developer.mozilla.org/en-US/docs/Web/API/HTMLHyperlinkElementUtils/search
    pub fn search(mut self, search: &str) -> Self {
        self.search = Some(search.into());
        self
    }

    /// Set the title (unimplemented in browsers)
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl From<String> for Url {
    // todo: Include hash and search.
    fn from(s: String) -> Self {
        let mut path: Vec<String> = s.split('/').map(|s2| s2.to_string()).collect();
        path.remove(0); // Remove a leading empty string.
        Self {
            path,
            hash: None,
            search: None,
            title: None,
        }
    }
}

/// Get the current url path, without a prepended /
fn get_path() -> String {
    let path = util::window()
        .location()
        .pathname()
        .expect_throw("Can't find pathname");
    path[1..path.len()].to_string() // Remove leading /
}

fn get_hash() -> String {
    let hash = util::window().location().hash().expect_throw("Can't find hash");
    hash.to_string().replace("#", "")
}

fn get_search() -> String {
    let search = util::window()
        .location()
        .search()
        .expect_throw("Can't find search");
    search.to_string().replace("?", "")
}

/// For setting up landing page routing. Unlike normal routing, we can't rely
/// on the popstate state, so must go off path, hash, and search directly.
pub fn initial<Ms>(update: impl Fn(Ms), routes: fn(&Url) -> Ms)
    where
        Ms: Clone + 'static,
{
    let raw_path = get_path();
    let path_ref: Vec<&str> = raw_path.split('/').collect();
    let path: Vec<String> = path_ref.into_iter().map(|p| p.to_string()).collect();

    let raw_hash = get_hash();
    let hash = match raw_hash.len() {
        0 => None,
        _ => Some(raw_hash),
    };

    let raw_search = get_search();
    let search = match raw_search.len() {
        0 => None,
        _ => Some(raw_search),
    };

    let url = Url {
        path,
        hash,
        search,
        title: None,
    };

    update(routes(&url));
}

fn remove_first(s: &str) -> Option<&str> {
    s.chars().next().map(|c| &s[c.len_utf8()..])
}

fn clean_url(mut url: Url) -> Url {
    let mut cleaned_path = vec![];
    for part in &url.path {
        if let Some(first) = part.chars().next() {
            if first == '/' {
                cleaned_path.push(remove_first(part).unwrap_throw().to_string());
            } else {
                cleaned_path.push(part.to_string());
            }
        }
    }

    url.path = cleaned_path;
    url
}

/// Add a new route using history's push_state method.
///https://developer.mozilla.org/en-US/docs/Web/API/History_API
pub fn push_route(mut url: Url) {
    // Purge leading / from each part, if it exists, eg passed by user.
    url = clean_url(url);

    // We use data to evaluate the path instead of the path displayed in the url.
    let data =
        JsValue::from_serde(&serde_json::to_string(&url).expect_throw("Problem serializing route data"))
            .expect_throw("Problem converting route data to JsValue");

    // title is currently unused by Firefox.
    let title = match url.title {
        Some(t) => t,
        None => "".into(),
    };

    // Prepending / means replace
    // the existing path. Not doing so will add the path to the existing one.
    let path = String::from("/") + &url.path.join("/");

    util::window()
        .history()
        .expect_throw("Can't find history")
        .push_state_with_url(&data, &title, Some(&path))
        .expect_throw("Problem pushing state");

    // Must set hash and search after push_state, or the url will be overwritten.
    let location = util::window().location();

    if let Some(hash) = url.hash {
        location.set_hash(&hash).expect_throw("Problem setting hash");
    }

    if let Some(search) = url.search {
        location
            .set_search(&search)
            .expect_throw("Problem setting search");
    }
}

/// A convenience function, for use when only a path is required.
pub fn push_path<T: ToString>(path: Vec<T>) {
    push_route(Url::new(path));
}

/// Add a listener that handles routing for navigation events like forward and back.
pub fn setup_popstate_listener<Ms>(
    update: impl Fn(Ms) + 'static,
    update_ps_listener: impl Fn(Closure<FnMut(web_sys::Event)>) + 'static,
    routes: fn(&Url) -> Ms)
    where
        Ms: Clone + 'static,
{
    let closure = Closure::wrap(Box::new(move |ev: web_sys::Event| {
        let ev = ev
            .dyn_ref::<web_sys::PopStateEvent>()
            .expect_throw("Problem casting as Popstate event");

        let url: Url = match ev.state().as_string() {
            Some(state_str) => {
                serde_json::from_str(&state_str).expect_throw("Problem deserializing popstate state")
            }
            // This might happen if we go back to a page before we started routing. (?)
            None => {
                let empty: Vec<String> = Vec::new();
                Url::new(empty)
            }
        };

        update(routes(&url));
    }) as Box<FnMut(web_sys::Event) + 'static>);

    (util::window().as_ref() as &web_sys::EventTarget)
        .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())
        .expect_throw("Problem adding popstate listener");

    update_ps_listener(closure);
}

/// Set up a listener that intercepts clicks on elements containing an Href attribute,
/// so we can prevent page refreshfor internal links, and route internally.  Run this on load.
pub fn setup_link_listener<Ms>(
    update: impl Fn(Ms) + 'static,
    routes: fn(&Url) -> Ms,
) where
    Ms: Clone + 'static
{
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
        if let Some(et) = event.target() {
            if let Some(el) = et.dyn_ref::<web_sys::Element>() {
                let tag = el.tag_name();
                // Base and Link tags use href for something other than navigation.
                if tag == "Base" || tag == "Link" {
                    return;
                }
                // todo use anchor element/property?
                if let Some(href) = el.get_attribute("href") {
                    if let Some(first) = href.chars().next() {
                        // The first character being / indicates a rel link, which is what
                        // we're intercepting.
                        // todo: Handle other cases that imply a relative link.
                        if first != '/' {
                            return;
                        }
                        event.prevent_default(); // Prevent page refresh
                        // Route internally based on href's value
                        let url = href.into();
                        update(routes(&url));
                        push_route(url);
                    }
                }
            }
        }
    }) as Box<FnMut(web_sys::Event) + 'static>);

    (util::document().as_ref() as &web_sys::EventTarget)
        .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
        .expect_throw("Problem setting up link interceptor");

    closure.forget(); // todo: Can we store the closure somewhere to avoid using forget?
}
