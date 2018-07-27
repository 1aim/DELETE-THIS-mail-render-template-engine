extern crate mail_types as mail;
extern crate mail_common as common;
extern crate mail_headers as headers;
extern crate mail_template as template;

#[macro_use]
extern crate failure;
extern crate mime as media_type;
extern crate futures;
extern crate soft_ascii_string;
#[macro_use]
extern crate vec1;
extern crate conduit_mime_types;
#[macro_use]
extern crate lazy_static;
extern crate serde;


#[cfg(any(feature="tera-engine", feature="handlebars-engine"))]
#[macro_use]
extern crate serde_derive;
#[cfg(feature="tera-engine")]
extern crate tera as tera_crate;
#[cfg(feature="handlebars-engine")]
extern crate handlebars as handlebars_crate;

// ordered by possible "dependentness",
// any module further down in the list
// can import from any module above it.
// But a module depending on a module later
// in the ordering _should_ not happen.
pub mod error;
mod utils;
mod settings;
mod spec;
//TODO rename
#[macro_use]
mod traits;
mod rte;
#[cfg(feature="tera-engine")]
pub mod tera;
#[cfg(feature="handlebars-engine")]
pub mod handlebars;

pub use self::settings::*;
pub use self::spec::*;
pub use self::traits::*;
pub use self::rte::*;