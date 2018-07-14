use std::collections::HashMap;

use mail::{Resource, Context};
use mail::file_buffer::FileBuffer;

use template::TemplateEngine;
use template::{
    EmbeddedWithCId,
    BodyPart, MailParts
};

use ::error::InsertionError;
use ::utils::fix_newlines;
use ::spec::TemplateSpec;
use ::traits::{RenderEngine, RenderEngineBase, AdditionalCIds};

#[derive(Debug)]
pub struct RenderTemplateEngine<R>
    where R: RenderEngineBase
{
    fix_newlines: bool,
    render_engine: R,
    id2spec: HashMap<String, TemplateSpec>,
    global_embeddings: HashMap<String, EmbeddedWithCId>
}


impl<R> RenderTemplateEngine<R>
    where R: RenderEngineBase
{

    pub fn new(render_engine: R) -> Self {
        RenderTemplateEngine {
            render_engine,
            id2spec: Default::default(),
            fix_newlines: !R::PRODUCES_VALID_NEWLINES,
            global_embeddings: HashMap::new()
        }
    }

    pub fn set_fix_newlines(&mut self, should_fix_newlines: bool) {
        self.fix_newlines = should_fix_newlines
    }

    pub fn does_fix_newlines(&self) -> bool {
        self.fix_newlines
    }

    /// add a global embedding
    ///
    /// A global embedding wil be made implicitly available for each render call
    /// through the `AdditionalCids` instance.
    ///
    /// If a template specifies an embedding with the same name as an global
    /// embedding it will shadow the global embedding.
    ///
    /// Note that while the `name` can be any string, many render engines only
    /// handle names which are valid idents (or at last only handle this values
    /// in an nice way). Also what exactly an ident is depends on the render engine,
    /// too. What always should work is a name starting with an ascii alphabetic
    /// letter followed by any not to large number of other ascii alphanumeric
    /// letters including `'_'`.
    pub fn add_global_embedding(&mut self, name: String, value: EmbeddedWithCId) -> Option<EmbeddedWithCId> {
        self.global_embeddings.insert(name, value)
    }

    /// remove a global embedding
    ///
    /// If no embedding with the given name exists `None` is returned.
    pub fn remove_global_embedding(&mut self, name: &str) -> Option<EmbeddedWithCId> {
        self.global_embeddings.remove(name)
    }

    /// access `name -> embedding` mapping of global embeddings
    pub fn global_embeddings(&self) -> &HashMap<String, EmbeddedWithCId> {
        &self.global_embeddings
    }

    /// add a `TemplateSpec`, loading all templates in it
    ///
    /// If a template with the same name is contained it
    /// will be removed (and unloaded and returned).
    ///
    /// If a template replaces a new template the old
    /// template is first unloaded and then the new
    /// template is loaded.
    ///
    /// # Error
    ///
    /// If the render templates where already loaded or can not
    /// be loaded an error is returned.
    ///
    /// If an error occurs when loading a new spec which _replaces_
    /// an old spec the old spec is already removed and unloaded.
    /// I.e. it's guaranteed that if `insert` errors there will no
    /// longer be an template associated with the given id.
    ///
    pub fn insert_spec(
        &mut self,
        id: String,
        spec: TemplateSpec
    ) -> Result<Option<TemplateSpec>, InsertionError<R::LoadingError>> {
        use std::collections::hash_map::Entry::*;
        match self.id2spec.entry(id) {
            Occupied(mut entry) => {
                let old = entry.insert(spec);
                self.render_engine.unload_templates(&old);
                let res = self.render_engine.load_templates(entry.get());
                if let Err(error) = res {
                    let (_, failed_new_value) = entry.remove_entry();
                    Err(InsertionError {
                        error, failed_new_value,
                        old_value: Some(old)
                    })
                } else {
                    Ok(Some(old))
                }
            },
            Vacant(entry) => {
                let res = self.render_engine.load_templates(&spec);
                if let Err(error) = res {
                    Err(InsertionError {
                        error, failed_new_value: spec,
                        old_value: None
                    })
                } else {
                    entry.insert(spec);
                    Ok(None)
                }
            }
        }
    }

    /// removes and unload the spec associated with the given id
    ///
    /// If no spec is associated with the given id nothing is done
    /// (and `None` is returned).
    pub fn remove_spec(&mut self, id: &str) -> Option<TemplateSpec> {
        let res =  self.id2spec.remove(id);
        if let Some(spec) = res.as_ref() {
            self.render_engine.unload_templates(spec);
        }
        res
    }

    pub fn specs(&self) -> &HashMap<String, TemplateSpec> {
        &self.id2spec
    }

    pub fn lookup_spec(&self, template_id: &str) -> Option<&TemplateSpec> {
        self.id2spec.get(template_id)
    }

}

impl<C, D, R> TemplateEngine<C, D> for RenderTemplateEngine<R>
    where C: Context, R: RenderEngine<D>
{
    type TemplateId = str;
    type Error = <R as RenderEngineBase>::RenderError;

    fn use_template(
        &self,
        template_id: &str,
        data: &D,
        ctx: &C,
    ) -> Result<MailParts, Self::Error >
    {
        let spec = self.lookup_spec(template_id)
            .ok_or_else(|| R::unknown_template_id_error(template_id))?;

        //OPTIMIZE there should be a more efficient way
        // maybe use Rc<str> as keys? and Rc<Resource> for embeddings?
        let shared_embeddings = spec.embeddings().iter()
            .map(|(key, resource)| create_embedding(key, resource, ctx))
            .collect::<HashMap<_,_>>();

        let bodies = spec.sub_specs().try_mapped_ref(|sub_spec| {

            let embeddings = sub_spec.embeddings().iter()
                .map(|(key, resource)| create_embedding(key, resource, ctx))
                .collect::<HashMap<_,_>>();

            let rendered = {
                let embeddings = &[&embeddings, &shared_embeddings, self.global_embeddings()];
                let additional_cids = AdditionalCIds::new(embeddings);
                self.render_engine.render(sub_spec, data, additional_cids)?
            };

            let rendered =
                if self.fix_newlines {
                    fix_newlines(rendered)
                } else {
                    rendered
                };

            let buffer = FileBuffer::new(sub_spec.media_type().clone(), rendered.into());
            let resource = Resource::sourceless_from_buffer(buffer);

            Ok(BodyPart {
                resource: resource,
                embeddings: embeddings.into_iter().map(|(_,v)| v).collect()
            })
        })?;

        let attachments = spec.attachments().iter()
            .map(|resource| EmbeddedWithCId::attachment(resource.clone(), ctx))
            .collect();

        Ok(MailParts {
            alternative_bodies: bodies,
            //TODO collpas embeddings and attachments and use their disposition parma
            // instead
            shared_embeddings: shared_embeddings.into_iter().map(|(_, v)| v).collect(),
            attachments,
        })
    }
}

fn create_embedding(
    key: &str,
    resource: &Resource,
    ctx: &impl Context
) -> (String, EmbeddedWithCId)
{
    (key.to_owned(), EmbeddedWithCId::inline(resource.clone(), ctx))
}