use std::fmt::{self, Display};
use handlebars_crate::TemplateError;

#[derive(Failure)]
pub enum LoadingError {
    #[fail(display="template id is used multiple times for different templates: {}", id)]
    TemplateIdCollision { id: String },

    #[fail(display="can not add free template as template id is used by non-free template: {}", id)]
    FreeTemplateIdCollision { id: String }

    #[fail(display="{}", _0)]
    TemplateParsing(TemplateError)
}

