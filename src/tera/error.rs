use failure::Backtrace;
use tera_crate;


#[derive(Debug, Fail)]
pub enum TeraError {

    #[fail(display="unknown template id: {}", id)]
    UnknowTemplateId { id: String },

    #[fail(display="template id is used multiple times for different templates: {}", id)]
    TemplateIdCollision { id: String },

    #[fail(display="{}", kind)]
    RenderError {
        kind: tera_crate::ErrorKind,
        backtrace: Backtrace
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