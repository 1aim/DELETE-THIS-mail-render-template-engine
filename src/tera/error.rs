use failure::Backtrace;
use tera_crate;

use ::FromUnknownTemplateId;

#[derive(Debug, Fail)]
pub enum TeraError {

    #[fail(display="unknown template id: {}", id)]
    UnknowTemplateId { id: String },

    #[fail(display="{}", kind)]
    RenderError {
        kind: tera_crate::ErrorKind,
        backtrace: Backtrace
    }
}


impl FromUnknownTemplateId<str> for TeraError {
    fn from_unknown_template_id(template_id: &str) -> Self {
        TeraError::UnknowTemplateId { id: template_id.to_owned() }
    }
}


//TODO/BUG actually impl a real from
impl From<tera_crate::Error> for TeraError {
    fn from(err: tera_crate::Error) -> Self {
        let tera_crate::Error(kind, _state) = err;
        TeraError::RenderError {
            kind,
            backtrace: Backtrace::new()
        }
    }
}