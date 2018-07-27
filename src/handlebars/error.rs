use std::{io as std_io};
use handlebars_crate::{TemplateError, TemplateFileError};

#[derive(Debug, Fail)]
pub enum LoadingError {
    #[fail(display="template id is used multiple times for different templates: {}", id)]
    TemplateIdCollision { id: String },

    #[fail(display="can not add free template as template id is used by non-free template: {}", id)]
    FreeTemplateIdCollision { id: String },

    #[fail(display="{}", _0)]
    TemplateParsing(TemplateError),

    #[fail(display="Template {}: {}", template, err)]
    Io { err: std_io::Error, template: String }
}

impl From<TemplateError> for LoadingError {
    fn from(err: TemplateError) -> Self {
        LoadingError::TemplateParsing(err)
    }
}

impl From<std_io::Error> for LoadingError {
    fn from(err: std_io::Error) -> Self {
        LoadingError::Io { err, template: "<anonym>".to_owned() }
    }
}

impl From<TemplateFileError> for LoadingError {
    fn from(err: TemplateFileError) -> Self {
        match err {
            TemplateFileError::TemplateError(tple) =>
                tple.into(),
            TemplateFileError::IOError(err, template) =>
                LoadingError::Io { err, template }
        }
    }
}
