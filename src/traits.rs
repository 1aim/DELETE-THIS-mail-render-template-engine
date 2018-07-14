use std::collections::{HashMap, HashSet};

use failure::Fail;
use serde::{Serializer, Serialize};
use headers::components::ContentId;
use template::EmbeddedWithCId;

use ::spec::{TemplateSpec, SubTemplateSpec};

/// Trait implemented by any `RenderEngine`
///
/// As the the `RenderEngine` trait is generic over
/// `D` but some parts are independent of `D` i.e.
/// are meant to be the same for any `D` the functionality
/// was separated into this trait.
pub trait RenderEngineBase {
    const PRODUCES_VALID_NEWLINES: bool;

    /// Error which can be produced when rendering a
    /// template (through the `RenderEngine` trait)
    type RenderError: Fail;

    /// Error which can be produced when loading a
    /// template.
    type LoadingError: Fail;

    /// loads the templates associated with the given spec.
    ///
    ///
    /// # Error
    ///
    /// If a collision with a render template id occurs
    /// an error is returned.
    ///
    /// If the template(s) can not be loaded for any reason
    /// an error is returned. Reasons can include, but might
    /// not be limited to:
    ///
    /// - the template file is missing
    /// - the template in the file is malformed
    /// - permissions to read the file are missing
    ///
    fn load_templates(&mut self, spec: &TemplateSpec) -> Result<(), Self::LoadingError>;

    /// unloads templates (if loaded)
    ///
    /// If the templates associated with `spec` are loaded
    /// this will unload them, if not this won't do anything.
    ///
    /// This can be used to reload a templates.
    fn unload_templates(&mut self, spec: &TemplateSpec);

    /// create a error representing that not template for given id was found
    ///
    /// Note that the id is _not_ a template name but the id of an
    /// `TemplateSpec` which might be associated with multiple
    /// templates. Through returning the same error as if a
    /// specific template was missing is normally fine, as a
    /// `TemplateSpec` can be seen as a form of big template
    /// split into multiple smaller templates which are rendered
    /// separately and then glued together again.
    fn unknown_template_id_error(id: &str) -> Self::RenderError;
}



/// Trait providing the `render` function.TemplateSpec
///
/// This type is generic over `D` as render is not necessary
/// implemented for any `D` but just for any `D` which implements
/// some specific bounds, but this bounds are dependent on
/// the actual implementation of `RenderEngine`, so it can't
/// but part of the `render` function signature.
///
/// E.g. it is possible to implement `RenderEngine` for any `D`
/// where `D: Serialize` or e.g. `D: ATemplateThingy` allowing this
/// implementation to be no
pub trait RenderEngine<D>: RenderEngineBase {

    fn render(
        &self,
        template: &SubTemplateSpec,
        data: &D,
        additional_cids: AdditionalCIds
    ) -> Result<String, <Self as RenderEngineBase>::RenderError>;

}


/// A type aggregating multiple `String => EmbeddedWithCId` mappings
///
/// There is a variable amount of sources defining `String => EmbeddedWithCId`
/// mappings which _should_ not overlap. If they do overlap the value of
/// the first hashmap containing the key is used and later maps are ignored.
///
/// It allows template engines to present a single `cid` (or similar)
/// field through which all template provided `cid` can be accessed
/// through their name.
pub struct AdditionalCIds<'a> {
    additional_resources: &'a [&'a HashMap<String, EmbeddedWithCId>]
}

impl<'a> AdditionalCIds<'a> {

    pub fn new(additional_resources: &'a [&'a HashMap<String, EmbeddedWithCId>]) -> Self {
        AdditionalCIds { additional_resources }
    }


    /// returns the content id associated with the given name
    ///
    /// If multiple of the maps used to create this type contain the
    /// key the first match is returned and all later ones are ignored.
    pub fn get(&self, name: &str) -> Option<&ContentId> {
        for possible_source in self.additional_resources {
            if let Some(res) = possible_source.get(name) {
                return Some(res.content_id());
            }
        }
        return None;
    }
}

impl<'a> Serialize for AdditionalCIds<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut existing_keys = HashSet::new();
        serializer.collect_map(
            self.additional_resources
            .iter()
            .flat_map(|m| m.iter().map(|(k, v)| (k, v.content_id())))
            .filter(|key| existing_keys.insert(key.to_owned()))
        )
    }
}



