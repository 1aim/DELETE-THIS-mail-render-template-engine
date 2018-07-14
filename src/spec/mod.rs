use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::mem::replace;

use vec1::Vec1;

use mail::Resource;
use headers::components::MediaType;

use ::error::CreatingSpecError;
use ::utils::{new_string_path, check_string_path};
use ::settings::LoadSpecSettings;

mod from_dir;

/// A type representing a (mail) Template
///
/// It consists of:
///
/// - A number of sub-templates
///   (one for each alternate body).
///
/// - A number of embeddings accessible by name (any string,
///   but depending on the template engine this might
///   have to be a valid ident).
///
/// - It can specify number of attachments which should
///   always be added if the template is used.
///
/// - It also has an optional `base_path` which is
///   the root folder it was loaded from using `from_dir`.
#[derive(Debug)]
pub struct TemplateSpec {
    /// the `base_path` which was used to construct the template from,
    /// e.g. with `TemplateSpec::from_dir` and which is used for reloading
    base_path: Option<PathBuf>,
    /// one sub-template for each alternate body
    templates: Vec1<SubTemplateSpec>,
    /// template level embeddings, i.e. embeddings shared between alternative bodies
    embeddings: HashMap<String, Resource>,
    /// attachments to always add if this template is used
    attachments: Vec<Resource>
}

impl TemplateSpec {

    /// Derive a `TemplateSpec` from a folder based on it's content.
    ///
    /// This will use the folder name as the templates id/name.
    /// Then it will iterate over all sub-folders and treat each
    /// of them as a source for a sub-template, where the sub-folder
    /// name specifies the media type to use (through mapping it
    /// in settings e.g. `"text" -> "text/plain; charset=utf-8"`).
    ///
    /// In each sub-folder it looks for a `mail.*` file and uses
    /// it as the templates source code, any other file in it is
    /// used as an additional alt-body specific embedding.
    ///
    /// Additional files in the templates folder are interpreted
    /// as additional non body specific embeddings.
    ///
    /// Currently the implementation is slightly limited, in the
    /// future it should be extended to allow some configuration
    /// through something like `__spec__.toml` in the templates folder.
    ///
    /// # Example
    ///
    /// **example of an _templates_ dictionary tree containing _one_
    /// template with the name `templateA`**
    ///
    /// ```no_rust
    /// templates/
    ///  templateA/
    ///   html/
    ///     mail.html
    ///     emb_logo.png
    ///   text/
    ///     mail.text
    /// ```
    ///
    /// # Uniqueness of names
    ///
    /// Each name of a file use for an embedding should be unique in it's name
    /// part. So the html alternate body folder should not contain both `name.png`
    /// and `name.jpeg` at the same time as both are accessed through `name`.
    /// As this names are solely used to access the generated `Content-Id` in
    /// the templates this should not be much of an problem.
    ///
    /// # File name interpretation
    ///
    /// File names containing multiple "." are ambiguous in what part is
    /// the actual name and what part is a suffix. E.g. "this.is.a" could
    /// be interpreted as "this.is" with suffix "a" or as "this" with suffix
    /// "is.a". This crate treats everything before the first "." as the name
    /// and everything after as the suffix (se the name would be "this").
    ///
    /// This is also needed as the used render template engine might not
    /// support names containing a ".".
    ///
    ///
    #[inline]
    pub fn from_dir<P>(base_path: P, settings: &LoadSpecSettings)
        -> Result<TemplateSpec, CreatingSpecError>
        where P: AsRef<Path>
    {
        self::from_dir::from_dir(base_path.as_ref(), settings)
    }

    /// Derive a template from each dir in the dir specified by `templates_dir`
    pub fn from_dirs<P>(templates_dir: P, settings: &LoadSpecSettings)
        -> Result<Vec<(String, TemplateSpec)>, CreatingSpecError>
        where P: AsRef<Path>
    {
        self::from_dir::from_dirs(templates_dir.as_ref(), settings)
    }

    /// creates a new Template from a list of sub-templates (for alternate bodies)
    pub fn new(templates: Vec1<SubTemplateSpec>) -> Self {
        Self::new_with_embeddings(templates, Default::default())
    }

    /// creates a new Template from a list of sub-templates and embeddings
    pub fn new_with_embeddings(
        templates: Vec1<SubTemplateSpec>,
        embeddings: HashMap<String, Resource>
    ) -> Self {
        TemplateSpec {
            base_path: None,
            templates, embeddings,
            attachments: Vec::new()
        }
    }

    /// creates a new Template from a list of sub-templates and a base path
    pub fn new_with_base_path<P>(templates: Vec1<SubTemplateSpec>, base_path: P)
        -> Result<Self, CreatingSpecError>
        where P: AsRef<Path>
    {
        Self::new_with_embeddings_and_base_path(
            templates, Default::default(), base_path.as_ref()
        )
    }

    /// creates a new Template from a list of sub-templates, embedding mappings and a base path
    pub fn new_with_embeddings_and_base_path<P>(
        templates: Vec1<SubTemplateSpec>,
        embeddings: HashMap<String, Resource>,
        base_path: P
    ) -> Result<Self, CreatingSpecError>
        where P: AsRef<Path>
    {
        let path = base_path.as_ref().to_owned();
        check_string_path(&*path)?;
        Ok(TemplateSpec {
            base_path: Some(path),
            templates, embeddings,
            attachments: Vec::new()
        })
    }

    pub fn sub_specs(&self) -> &Vec1<SubTemplateSpec> {
        &self.templates
    }

    pub fn sub_specs_mut(&mut self) -> &mut Vec1<SubTemplateSpec> {
        &mut self.templates
    }

    pub fn embeddings(&self) -> &HashMap<String, Resource> {
        &self.embeddings
    }

    pub fn embeddings_mut(&mut self) -> &mut HashMap<String, Resource> {
        &mut self.embeddings
    }

    pub fn base_path(&self) -> Option<&Path> {
        self.base_path.as_ref().map(|r| &**r)
    }

    pub fn set_base_path<P>(&mut self, new_path: P) -> Result<Option<PathBuf>, CreatingSpecError>
        where P: AsRef<Path>
    {
        let path = new_path.as_ref();
        check_string_path(path)?;
        Ok(replace(&mut self.base_path, Some(path.to_owned())))
    }

    pub fn attachments(&self) -> &Vec<Resource> {
        &self.attachments
    }

    pub fn attachments_mut(&mut self) -> &mut Vec<Resource> {
        &mut self.attachments
    }

}

/// A type representing the part of a template which represents a alternate mail body
///
/// This type contains a way to get a specific templates source (e.g.
/// a the content of an specific handlebars file) the media type which
/// this alternate body should have, and a mappings of embeddings specific
/// to this alternate body
#[derive(Debug)]
pub struct SubTemplateSpec {
    media_type: MediaType,
    source: TemplateSource,
    // (Name, Resource) | name is used by the template engine e.g. log, and differs to
    // resource spec use_name which would
    //  e.g. be logo.png but referring to the file long_logo_name.png
    embeddings: HashMap<String, Resource>,//todo use insert order keeping map
}

impl SubTemplateSpec {

    //FIXME to many arguments alternatives: builder,
    // default values (embedding, attachment)+then setter,
    // default values + then with_... methods
    pub fn new<P>(path: P,
                  media_type: MediaType,
                  embeddings: HashMap<String, Resource>,
    ) -> Result<Self, CreatingSpecError>
        where P: AsRef<Path>
    {
        let source = TemplateSource::Path(new_string_path(path.as_ref())?);
        Ok(SubTemplateSpec::new_with_template_source(source, media_type, embeddings))
    }

    pub fn new_with_template_source(
        source: TemplateSource,
        media_type: MediaType,
        embeddings: HashMap<String, Resource>
    ) -> Self {
        SubTemplateSpec { source, media_type, embeddings }
    }

    pub fn source(&self) -> &TemplateSource {
        &self.source
    }

    pub fn set_source(&mut self, source: TemplateSource) -> TemplateSource {
        replace(&mut self.source, source)
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn set_media_type(&mut self, media_type: MediaType) -> MediaType {
        //we might want to add restrictions at some point,e.g. no multi-part media type
        replace(&mut self.media_type, media_type)
    }

    pub fn embeddings(&self) -> &HashMap<String, Resource> {
        &self.embeddings
    }

    pub fn embedding_mut(&mut self) -> &mut HashMap<String, Resource> {
        &mut self.embeddings
    }

}


/// Describes how to get the source of an render template.
///
/// Available method currently contain:
///
/// - reading the source from a file specified by an path
/// - the source is directly given as an `String`
///
#[derive(Debug, Clone)]
pub enum TemplateSource {
    //TODO have some `StringPath` type
    /// This uses string paths as the render engine might want to uses
    /// the path as an Id for looking up the template.
    ///
    /// For simplicity the path is not relative to the `TemplateSpec.base_path`
    /// but to the working directory (if it is relative). This means it also
    /// normally contains the `base_path`, if there is one.
    Path(String),

    /// A string representing the source of a template, e.g. for a
    /// handlebars-like render engine this could be "Hy {{name}}"
    Source {
        /// a **unique** id which the render engine can associate
        /// the parsed template with
        id: String,
        /// the string representing the source
        content: String
    }
}

impl TemplateSource {

    /// returns the id for this source
    ///
    /// - If the source if a `Path` the id _is_
    /// the path (as string).
    ///
    /// - If the source is a source string the id
    ///   specified in the `Source` variant is used.
    pub fn id(&self) -> &str {
        use self::TemplateSource::*;
        match *self {
            Path(ref path_is_id) => &path_is_id,
            Source { ref id, .. } => &id
        }
    }
}