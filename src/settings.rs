use std::collections::HashMap;
use std::path::Path;

use failure::Fail;
use media_type::CHARSET;
use vec1::Vec1;

use headers::components::MediaType;

use ::error::{LoadingSpecError, LoadingSpecErrorVariant};
use ::utils;


//Type::find_media_type(Path)
//   combines type e.g. for html => application/html
//   with looking at the file ending, and more?
//   needs:
//      the name -> media_type mapping
//      some form of charset setting/detection
//Type::alias_names()
//   list of alias names e.g. mail.html, mail.htm,
//   needs:
//      list of aliases file endings

lazy_static! {
    pub static ref DEFAULT_SETTINGS: LoadSpecSettings = {
        let html =  Type {
            base_type: "text".to_owned(),
            base_subtype: "html".to_owned(),
            suffixes: vec1![ ".html".to_owned(), ".htm".to_owned() ],
            charset: Some("utf-8".to_owned()),
        };
        let xhtml = Type {
            base_type: "application".to_owned(),
            base_subtype: "xhtml+xml".to_owned(),
            suffixes: vec1![ ".xhtml".to_owned(), ".xml".to_owned() ],
            charset: Some("utf-8".to_owned()),
        };
        let enriched = Type {
            base_type: "text".to_owned(),
            base_subtype: "enriched".to_owned(),
            suffixes: vec1![ ".txt".to_owned(), ".text".to_owned() ],
            charset: Some("utf-8".to_owned()),
        };
        let text = Type {
            base_type: "text".to_owned(),
            base_subtype: "plain".to_owned(),
            suffixes: vec1![ ".txt".to_owned(), ".text".to_owned() ],
            charset: Some("utf-8".to_owned()),
        };

        let mut se = LoadSpecSettings::new();
        se.set_type_lookup("text", text, None).unwrap();
        se.set_type_lookup("enriched", enriched, Some("text")).unwrap();
        se.set_type_lookup("xhtml", xhtml, Some("enriched")).unwrap();
        se.set_type_lookup("html", html, Some("xhtml")).unwrap();

        se
    };
}


//IMPLEMENTATION NOTE: for now this is a simple configurabe think,
// BUT in the future it can either be
// 1. extended to support more stuff
// 2. made into a trait with multiple impl. where the current impl. is just the default

#[derive(Debug, Clone)]
pub struct LoadSpecSettings {
    type_lookup: HashMap<String, (usize, Type)>,
}

impl LoadSpecSettings {

    pub fn new() -> Self {
        LoadSpecSettings { type_lookup: HashMap::new() }
    }



    pub fn get_type(&self, name: &str) -> Option<&Type> {
        self.type_lookup.get(name)
            .map(|data| &data.1)
    }

    /// returns the type and its priority idx for a given name if there is a type registered for it
    ///
    /// Note that the priority idx can change if type lookups are inserted/removed.
    /// See `get_priority_idx` for a more indepth explanation
    /// of how to interprete the priority idx.
    pub fn get_type_with_priority(&self, name: &str) -> Option<(usize, &Type)> {
        self.type_lookup.get(name)
            .map(|data| (data.0, &data.1))
    }

    pub fn set_type_lookup<I>(
        &mut self, name: I, type_: Type, prioritize_over: Option<&str>
    ) -> Result<(), LoadingSpecError>
        where I: Into<String>
    {
        self._set_type_lookup(name.into(), type_, prioritize_over)
    }

    fn _set_type_lookup(&mut self, name: String, type_: Type, prioritize_over: Option<&str>)
        -> Result<(), LoadingSpecError>
    {
        let new_priority =
            if let Some(other) = prioritize_over {
                let other_prio = self.get_priority_idx(other)
                    .ok_or_else(|| LoadingSpecErrorVariant::NoMediaTypeFor { stem: other.to_owned() })?;
                other_prio + 1
            } else {
                0
            };


        let old_priority = self.get_priority_idx(&*name)
            .unwrap_or(self.type_lookup.len());

        //1. correct priorities before inserting the new one
        for data in self.type_lookup.values_mut() {
            let prio = data.0;

            // SAFE_MATH: that this can not underflow as if prio is 0 prio can not be > old_priority as
            // both are usize i.e. >= 0
            let updated_prio = prio
                // as we insert it everyons priority idxes >= the insertion prosition are now
                // greater by one.
                + if prio >= new_priority { 1 } else { 0 }
                // as we (potentially) remove it from it's old position the priority idxes > the
                // removed position are now smaller by one (if it has not been in type_lookup before
                // old_priority is `type_lookup.len()` i.e. > then all entries
                - if prio > old_priority  { 1 } else { 0 };

            data.0 = updated_prio;
        }

        // insert the new lookup potentailly replacing a existing one
        self.type_lookup.insert(name, (new_priority, type_));
        Ok(())
    }

    /// returns a priority index for the given type name, if there is a type registered for the name
    ///
    /// The priority index is can be seen as the index the type would have if it would be in a
    /// list of all registered types sorted by priority. **This also means that it can changes if
    /// new types are added**.
    ///
    /// The higher the priority idx is the higher is the priority, this is analog to the way
    /// `multipart/alternative` bodies are ordered, with the last body (~ highest priority idx)
    /// being the body which is preffered to be shown the most and the first body the one which
    /// should only be shown if all other bodies can not be displayed. Typical is a priority
    /// list like `[ text/plain, text/enriched, text/html ]`
    ///
    pub fn get_priority_idx(&self, name: &str) -> Option<usize> {
        self.type_lookup.get(name)
            .map(|data| data.0)
    }

    pub fn remove_type_lookup(&mut self, name: &str) -> Option<Type> {
        if let Some((old_priority, type_)) = self.type_lookup.remove(name) {
            for data in self.type_lookup.values_mut() {
                if data.0 > old_priority {
                    data.0 -= 1;
                }
            }
            Some(type_)
        } else {
            None
        }
    }


    #[inline]
    pub fn determine_media_type<P>(&self, path: P) -> Result<MediaType, LoadingSpecError>
        where P: AsRef<Path>
    {
        utils::sniff_media_type(path.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    base_type: String,
    base_subtype: String,
    suffixes: Vec1<String>,
    charset: Option<String>
}

impl Type {

    pub fn to_media_type_for<P>(&self, path: P) -> Result<MediaType, LoadingSpecError>
        where P: AsRef<Path>
    {
        self._as_media_type_for(path.as_ref())
    }

    fn _as_media_type_for(&self, _path: &Path) -> Result<MediaType, LoadingSpecError> {
        //FEAT: consider charset sniffing or validate sniffing, allow other parameters for more
        // unusual bodies
        // for now this is just creating a media type and set a preset charset,
        // not trying to verify the charset or anything else
        let media_type_res =
            if let Some(charset) = self.charset.as_ref() {
                MediaType::new_with_params(&self.base_type, &self.base_subtype, vec![
                    (CHARSET, charset)
                ])
            } else {
                MediaType::new(&self.base_type, &self.base_subtype)
            };

        let media_type = media_type_res
            .map_err(|err| err.context(LoadingSpecErrorVariant::BodyMediaTypeCreationFailure))?;

        Ok(media_type)
    }

    pub fn suffixes(&self) -> &Vec1<String> {
        &self.suffixes
    }

    pub fn template_base_name(&self) -> &str {
        "mail"
    }
}


#[cfg(test)]
mod test {
    use super::{LoadSpecSettings, Type};

    fn dumy_settings() -> LoadSpecSettings {
        let mut se = LoadSpecSettings::new();
        let text = dumy_type("text", "txt");
        se.set_type_lookup("text", text.clone(), None).unwrap();
        let xhtml = dumy_type("xhtml+xml", "xhtml");
        se.set_type_lookup("xhtml", xhtml.clone(), Some("text")).unwrap();
        let html = dumy_type("html", "html");
        se.set_type_lookup("html", html.clone(), Some("xhtml")).unwrap();
        se
    }

    fn dumy_type(subtype: &str, suffix: &str) -> Type {
        Type {
            base_type: "text".to_owned(),
            base_subtype: subtype.to_owned(),
            suffixes: vec1![ suffix.to_owned() ],
            charset: Some("utf-8".to_owned()),
        }
    }

    #[test]
    fn add_types_and_aliases() {
        let mut se = LoadSpecSettings::new();
        let text = dumy_type("text", "txt");
        se.set_type_lookup("text", text.clone(), None).unwrap();
        let html = dumy_type("html", "html");
        se.set_type_lookup("html", html.clone(), Some("text")).unwrap();
        let xhtml = dumy_type("xhtml+xml", "xhtml");
        se.set_type_lookup("xhtml", xhtml.clone(), Some("text")).unwrap();

        assert_eq!(se.get_type("text"), Some(&text));
        assert_eq!(se.get_type("xhtml"), Some(&xhtml));
        assert_eq!(se.get_type("html"), Some(&html));
        assert_eq!(se.get_priority_idx("text"), Some(0));
        assert_eq!(se.get_priority_idx("xhtml"), Some(1));
        assert_eq!(se.get_priority_idx("html"), Some(2));
        assert_eq!(se.get_type_with_priority("text"), Some((0, &text)));
        assert_eq!(se.get_type_with_priority("xhtml"), Some((1, &xhtml)));
        assert_eq!(se.get_type_with_priority("html"), Some((2, &html)));
    }



    #[test]
    fn override_type_different_priority() {
        let mut settings = dumy_settings();
        //give xhtml the least priority, and change suffix to xml
        settings.set_type_lookup("xhtml", dumy_type("xhtml+xml", "xml"), None).unwrap();

        let type_ = settings.get_type("xhtml").unwrap();
        assert_eq!(type_.suffixes(), &["xml".to_owned()])

    }

    #[test]
    fn remove_type() {
        let mut se = dumy_settings();
        se.remove_type_lookup("xhtml");

        assert_eq!(se.get_type_with_priority("text"), Some((0, &dumy_type("text", "txt"))));
        assert_eq!(se.get_type_with_priority("xhtml"), None);
        assert_eq!(se.get_type_with_priority("html"), Some((1, &dumy_type("html", "html"))));
    }

}