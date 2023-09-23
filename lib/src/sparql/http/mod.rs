#[cfg(not(feature = "http-client"))]
mod dummy;
#[cfg(feature = "http-client")]
mod simple;

#[cfg(not(feature = "http-client"))]
pub use dummy::Client;
#[cfg(feature = "http-client")]
pub use simple::Client;
