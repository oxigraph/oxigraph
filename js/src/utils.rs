use js_sys::{AsyncIterator, Function, IteratorNext, Reflect, Symbol};
use std::pin::pin;
use std::task::{Context, Poll, ready};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[macro_export]
macro_rules! format_err {
    ($msg:literal $(,)?) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new(&format!($msg)))
    };
    ($fmt:literal, $($arg:tt)*) => {
        ::wasm_bindgen::JsValue::from(::js_sys::Error::new(&format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_warn {
    ($($t:tt)*) => ($crate::utils::warn(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub(crate) fn warn(s: &str);
}

pub fn try_async_iter(val: &JsValue) -> Result<Option<IntoAsyncIter>, JsValue> {
    let async_iter_fn = Reflect::get(val, &Symbol::async_iterator())?;
    let Ok(async_iter_fn) = async_iter_fn.dyn_into::<Function>() else {
        return Ok(None);
    };
    let Ok(iter) = async_iter_fn.call0(val)?.dyn_into::<AsyncIterator>() else {
        return Ok(None);
    };
    Ok(Some(IntoAsyncIter {
        js: iter,
        done: false,
        pending: None,
    }))
}

pub struct IntoAsyncIter {
    js: AsyncIterator,
    done: bool,
    pending: Option<JsFuture>,
}

impl IntoAsyncIter {
    pub fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<JsValue, JsValue>>> {
        if self.done {
            return Poll::Ready(None);
        }
        if self.pending.is_none() {
            self.pending = match self.js.next() {
                Ok(next_promise) => Some(JsFuture::from(next_promise)),
                Err(e) => {
                    self.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
            };
        }
        let next = ready!(pin!(self.pending.as_mut().unwrap()).poll(cx));
        self.pending = None; // We have finished polling the future
        let next = match next {
            Ok(next) => IteratorNext::from(next),
            Err(e) => {
                self.done = true;
                return Poll::Ready(Some(Err(e)));
            }
        };
        Poll::Ready(if next.done() {
            self.done = true;
            None
        } else {
            Some(Ok(next.value()))
        })
    }
}

pub fn make_iterator_iterable(value: impl Into<JsValue>) -> Result<JsValue, JsValue> {
    let value = value.into();
    let symbol_value = value.clone();
    Reflect::set(
        &value,
        &Symbol::iterator(),
        &Closure::<dyn Fn() -> JsValue>::new(move || symbol_value.clone()).into_js_value(),
    )?;
    Ok(value)
}

pub fn make_async_iterator_iterable(value: impl Into<JsValue>) -> Result<JsValue, JsValue> {
    let value = value.into();
    let symbol_value = value.clone();
    Reflect::set(
        &value,
        &Symbol::async_iterator(),
        &Closure::<dyn Fn() -> JsValue>::new(move || symbol_value.clone()).into_js_value(),
    )?;
    Ok(value)
}
