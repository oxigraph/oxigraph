#[cfg(not(feature = "http_client"))]
mod dummy;
#[cfg(feature = "http_client")]
mod simple;

#[cfg(not(feature = "http_client"))]
pub use dummy::Client;
#[cfg(feature = "http_client")]
pub use simple::Client;
