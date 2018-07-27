use std::collections::HashSet;
use std::path::Path;
use std::io::Read;
use std::ops::Deref;

use serde::Serialize;
use handlebars_crate::{
    Handlebars, RenderError,
    HelperDef, DecoratorDef
};

use ::{
    RenderEngineBase, RenderEngine,
    AdditionalCIds,
    TemplateSpec, SubTemplateSpec,
    TemplateSource
};


use self::error::LoadingError;


mod error;

/// Render Engine using Handlebars for rendering
///
/// # "free" templates?
///
/// Templates are loaded based on information contained in
/// `TemplateSpec`'s added to the `TemplateRenderEngine` using
/// this render engine. Through this render engine might contain
/// additional templates not bound to any spec, which are called
/// free templates in this documentation. Their main usage is to
/// use them for template inherence.
///
/// If a new free template is added through one of the `register_free_template_*`
/// functions the behavior mirrors that of their counterparts on  the `Handlebars`
/// type i.e. if the name wasn't used a new template is added and if it was used
/// the template is overwritten. But to prevent problems and unexpected behavior
/// this is just the cases for free-templates. If a free templates is added having
/// a name colliding with a non-free template an error is returned. The same is true
/// for the other way around, i.e. adding a non-free template with the same name
/// as a free template.
#[derive(Debug)]
pub struct HandlebarsRenderEngine {
    handlebars: Handlebars,
    free_templates: HashSet<String>
}

impl HandlebarsRenderEngine {

    /// create a new handlebars render engine
    ///
    /// This will enable the strict mode by default.
    pub fn new() -> Self {
        Handlebars::new().into()
    }

    /// sets handlebars strict mode
    ///
    /// The default of `HandlebarsRenderEngine` is to _enable_ it.
    /// But the default of handle bar is to not enable it.
    pub fn set_strict_mode(&mut self, enabled: bool) {
        self.handlebars.set_strict_mode(enabled)
    }

    /// get a mut reference to inner handlebars object
    ///
    /// Note that using some methods of the inner object
    /// like e.g. `register*`, `unregister*` can brake
    /// this instance in a potential silent and hard to
    /// track way.
    ///
    /// This is why it's prefixed with `__` and ends
    /// with `_dont_use_this`
    ///
    /// The main reason why this method exists is that
    /// handlebars might add new methods in future non-major
    /// released which might not have been proxied yet. If
    /// this is a case please make a pull-request. Another
    /// reason is some helper function which require a `&mut Handlebar`
    /// but this is risky, consider setting up a `Handlebar`
    /// object instead and then turning it into a `HandlebarsRenderEngine`.
    #[doc(hidden)]
    pub fn __inner_mut_dont_use_this(&mut self) -> &mut Handlebars {
        &mut self.handlebars
    }

    /// Registers a free template based on a string.
    ///
    /// Take a look at the type level documentation for more information
    /// about free templates and potential name collisions.
    pub fn register_free_template_string<S>(
        &mut self,
        name: &str,
        tpl: S
    ) -> Result<(), LoadingError>
        where S: AsRef<str>
    {
        let tpl = tpl.as_ref();
        self.insert_free_template(name, |hbs| Ok(hbs.register_template_string(name, tpl)?))
    }

    /// Registers a free partial.
    ///
    /// (Free) partials are currently treated the same as
    /// (free) templates.
    ///
    /// Take a look at the type level documentation for more information
    /// about free templates and potential name collisions.
    pub fn register_free_partial<S>(
        &mut self,
        name: &str,
        partial: S
    ) -> Result<(), LoadingError>
        where S: AsRef<str>
    {
        let partial = partial.as_ref();
        self.insert_free_template(name, |hbs| Ok(hbs.register_partial(name, partial)?))
    }

    /// Registers a free template based on the content of an file.
    ///
    /// Take a look at the type level documentation for more information
    /// about free templates and potential name collisions.
    pub fn register_free_template_file<P>(
        &mut self,
        name: &str,
        path: P
    ) -> Result<(), LoadingError>
        where P: AsRef<Path>
    {
        let path = path.as_ref();
        self.insert_free_template(name, |hbs| Ok(hbs.register_template_file(name, path)?))
    }

    // TODO I have to reproduce this function and can't just wrap it!
    // pub fn register_free_templates_directory<P>(
    //     &mut self,
    //     tpl_extension: &'static str,
    //     dir_path: P
    // ) -> Result<(), LoadingError>
    //     where P: AsRef<Path>
    // {
    //  TODO find out what exactly this does on how this exactly behaves
    // }

    /// Registers a free template read from an source.
    ///
    /// Take a look at the type level documentation for more information
    /// about free templates and potential name collisions.
    pub fn register_template_source(
        &mut self,
        name: &str,
        source: &mut Read
    ) -> Result<(), LoadingError> {
        self.insert_free_template(name, |hbs| Ok(hbs.register_template_source(name, source)?))
    }

    /// Unregister a free template if there is a free template with the given name.
    ///
    /// If there is...
    ///
    /// - no template with the given name
    /// - no free template (but a non free template)
    ///
    /// ... then nothing is done.
    pub fn unregister_free_template(&mut self, name: &str) {
        if self.free_templates.contains(name) {
            self.handlebars.unregister_template(name);
        }
    }

    /// Unregister all free templates
    pub fn clear_free_templates(&mut self) {
        for id in self.free_templates.drain() {
            self.handlebars.unregister_template(&id);
        }
    }

    /// Register an helper to the inner `Handlebars` instance.
    pub fn register_helper(
        &mut self,
        name: &str,
        def: Box<HelperDef + 'static>
    ) -> Option<Box<HelperDef + 'static>> {
        self.handlebars.register_helper(name, def)
    }

    /// Register an decorator to the inner `Handlebars` instance.
    pub fn register_decorator(
        &mut self,
        name: &str,
        def: Box<DecoratorDef + 'static>
    ) -> Option<Box<DecoratorDef + 'static>> {
        self.handlebars.register_decorator(name, def)
    }

    /// Register an escape fn to the inner `Handlebars` instance.
    pub fn register_escape_fn<F: 'static>(
        &mut self,
        escape_fn: F
    )
        where F: Fn(&str) -> String + Send + Sync
    {
        self.handlebars.register_escape_fn(escape_fn)
    }

    /// Unregister an escape fn from the inner `Handlebars` instance.
    pub fn unregister_escape_fn(&mut self) {
        self.handlebars.unregister_escape_fn()
    }

    fn check_new_free_template_name(&self, name: &str) -> Result<(), LoadingError> {
        if !self.free_templates.contains(name) && self.handlebars.get_template(name).is_some() {
            Err(LoadingError::FreeTemplateIdCollision { id: name.to_owned() })
        } else {
            Ok(())
        }
    }

    fn insert_free_template<F>(&mut self, name: &str, insert_fn: F) -> Result<(), LoadingError>
        where F: FnOnce(&mut Handlebars) -> Result<(), LoadingError>
    {
        self.check_new_free_template_name(name)?;
        let ok = insert_fn(&mut self.handlebars)?;
        self.free_templates.insert(name.to_owned());
        Ok(ok)
    }
}

impl RenderEngineBase for HandlebarsRenderEngine {

    /// templates might not use "\r\n" line endings
    const PRODUCES_VALID_NEWLINES: bool = false;

    type RenderError = RenderError;
    type LoadingError = LoadingError;

    fn load_templates(&mut self, spec: &TemplateSpec) -> Result<(), Self::LoadingError> {
        implement_load_helper! {
            input::<Handlebars>(spec, &mut self.handlebars);
            error(LoadingError);
            collision_error_fn(|id| { LoadingError::TemplateIdCollision { id } });
            has_template_fn(|hbs, id| { hbs.get_template(id).is_some() });
            remove_fn(|hbs, id| { hbs.unregister_template(id) });
            add_file_fn(|hbs, path| { Ok(hbs.register_template_file(path, path)?) });
            add_content_fn(|hbs, id, content| { Ok(hbs.register_template_string(id, content)?) });
        }
    }

    fn unload_templates(&mut self, spec: &TemplateSpec) {
        for sub_spec in spec.sub_specs() {
            self.handlebars.unregister_template(sub_spec.source().id());
        }
    }

    fn unknown_template_id_error(id: &str) -> Self::RenderError {
        RenderError::new(format!("*Mail* Template not found: {}", id))
    }
}

#[derive(Serialize)]
struct DataWrapper<'a,D: Serialize + 'a> {
    data: &'a D,
    cids: AdditionalCIds<'a>
}

impl<D> RenderEngine<D> for HandlebarsRenderEngine
    where D: Serialize
{

    fn render(&self, spec: &SubTemplateSpec, data: &D, cids: AdditionalCIds)
        -> Result<String, Self::RenderError>
    {
        let data = &DataWrapper { data, cids };
        let id = spec.source().id();
        Ok(self.handlebars.render(id, data)?)
    }
}

/// Turns a Handlebars into a HandlebarsRenderEngine
///
/// This will implicitly enable the strict mode.
impl From<Handlebars> for HandlebarsRenderEngine {
    fn from(mut handlebars: Handlebars) -> Self {
        let mut free_templates = HashSet::new();
        for name in handlebars.get_templates().keys() {
            free_templates.insert(name.clone());
        }
        handlebars.set_strict_mode(true);
        HandlebarsRenderEngine { handlebars, free_templates }
    }
}

impl Default for HandlebarsRenderEngine {
    fn default() -> Self {
        HandlebarsRenderEngine::new()
    }
}

impl Deref for HandlebarsRenderEngine {
    type Target = Handlebars;

    fn deref(&self) -> &Self::Target {
        &self.handlebars
    }
}