use std::path::Path;
use std::ffi::OsStr;
use std::process::Command;
use std::io;

use failure::Fail;

use conduit_mime_types::Types as TypesBySuffix;

use headers::components::MediaType;

use ::error::{CreatingSpecError, CreatingSpecErrorVariant};

lazy_static! {
    static ref TYPES_BY_SUFFIX: TypesBySuffix = {
        TypesBySuffix::new()
            .expect("embedded json data corrupted in conduit_mime_types")
    };
}

// pub(crate) fn string_path_set(field: &mut String, new_path: &Path) -> Result<PathBuf, CreatingSpecError> {
//     let path = new_string_path(new_path)?;
//     let old = replace(field, path);
//     Ok(PathBuf::from(old))
// }

pub(crate) fn new_string_path<S>(path: S) -> Result<String, CreatingSpecError>
    where S: AsRef<OsStr>
{
    new_str_path(&path.as_ref()).map(|s|s.to_owned())
}

pub(crate) fn new_str_path<S>(path: &S) -> Result<&str, CreatingSpecError>
    where S: AsRef<OsStr>
{
    let path = path.as_ref();
    if let Some(path) = path.to_str() {
        Ok(path)
    } else {
        Err(CreatingSpecErrorVariant::NonStringPath(path.to_owned().into()).into())
    }
}

pub(crate) fn check_string_path(path: &Path) -> Result<(), CreatingSpecError> {
    if path.to_str().is_none() {
        Err(CreatingSpecErrorVariant::NonStringPath(path.into()).into())
    } else {
        Ok(())
    }
}


pub(crate) fn sniff_media_type(path: &Path) -> Result<MediaType, CreatingSpecError> {
    //this does not work for
    // 1. multi part extensions like .tar.gz
    // 2. types not supported by conduit-media-types (which, btw. include .tar.gz /.tgz)
    let extension = path.extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| CreatingSpecErrorVariant::NoValidFileStem { file: path.into() })?;

    // 1. determine media type by file ending
    let by_extension_str_media_type = TYPES_BY_SUFFIX
        .get_mime_type(extension)
        .ok_or_else(|| CreatingSpecErrorVariant::NoMediaTypeFor { stem: extension.to_owned() })?;

    // 2. determine media type by file
    let media_type = sniff_with_file_cmd(path)?;

    // 3. compare (FEAT: validate instead of 2+3, as it only needs to run some tests)
    if media_type.full_type() != by_extension_str_media_type {
        //FEAT if they are close by in the subtyping base media tree it's still fine
        // e.g. if tp by extension is text/plain and tp by file is text/x-tex we can go with
        // text/plain (the file ending). But we might also do so the other way around _iff_
        // we could get the actual probabilities for both the extension based and the file ending based
        // one
        return Err(CreatingSpecErrorVariant::FileStemAndContentDifferInMediaType {
            path: path.into(),
            by_extension: by_extension_str_media_type.to_owned(),
            by_content: media_type.full_type().to_string()
        }.into());
    }

    Ok(media_type)

}


pub(crate) fn sniff_with_file_cmd(path: &Path) -> Result<MediaType, CreatingSpecError> {
    let out = Command::new("file")
        .args(&["-b", "--mime"])
        .arg(path)
        .output()?;

    if !out.status.success() {
        //note: this should normally not happen, expect if there is no file a <path>
        let err_msg = format!(
            concat!(
                "file -b -mime <path> failed with exit code {}\n",
                "path: {path}\n",
                "stderr: {stderr}\n",
                "stdout: {stdout}"
            ),
            path=path.display(),
            stderr=String::from_utf8_lossy(&out.stderr),
            stdout=String::from_utf8_lossy(&out.stdout)
        );
        return Err(io::Error::new(io::ErrorKind::Other, err_msg).into())
    }

    let media_type = String
        ::from_utf8(out.stdout)
        .map_err(|err| err.context(CreatingSpecErrorVariant::NonUtf8MediaType))
        .and_then(|string| {
            //FEAT: make parse accept owned data
            MediaType::parse(string.trim())
                .map_err(|err| err.context(CreatingSpecErrorVariant::NotAMediaType))
        })?;

    Ok(media_type)
}

/// replace any orphan \r,\n chars with \r\n if needed
///
/// If the there is no need to replace anything the input String will be returned,
/// if it's just a tailin \r the input string will be extended by \n and returned,
/// else a new string is created containing the input text but with all orphan CR/NL's
/// replaced with \r\n.
///
/// Note: this function was intentionally designed to not consume a &str and return a Cow<str>,
/// if it is ever made public the interface should be changed to that and the place where it is
/// used should be changed to match on a Cow returning the input if it is Cow::Borrowed or returning
/// the new value and droping the input if it is Cow::Owned
pub(crate) fn fix_newlines(text: String) -> String {
    let mut hit_cr = false;
    let offset = text.bytes().position(|bch| {
        match bch {
            b'\r' => {
                let invalid = hit_cr == true;
                hit_cr = true;
                invalid
            },
            b'\n' => {
                let invalid = hit_cr == false;
                hit_cr = false;
                invalid
            },
            _ => {
                hit_cr == true
            }
        }
    });


    if let Some(offset) = offset {
        _fix_newlines_from(&*text, offset)
    } else if hit_cr {
        let mut out = text;
        out.push('\n');
        out
    } else {
        text
    }
}

// note this expect offset to be a bad char i.e. if text[offset] is \n
// it assumes text[offset-1] was not \r and if it is any other char
// it assumes text[offset-1] has been \r.
// IT PANICS if there is no char at offser i.e. if text[offset..].chars().next() == None
fn _fix_newlines_from(text: &str, offset: usize) -> String {
    let mut buff = String::with_capacity(text.len() + 1);
    let (ok, tail) = text.split_at(offset);
    buff.push_str(ok);

    let mut chars = tail.chars();
    let mut hit_cr = false;
    // we know the first char is wrong
    match chars.next() {
        Some('\n') => {
            // \n is wrong if there was no preceeding \r
            buff.push('\r');
            buff.push('\n');
        },
        Some(not_nl) => {
            // not_nl incl \r is only wrong (without lookahead) if preceded by an \r
            buff.push('\n');
            buff.push(not_nl);
            hit_cr = not_nl == '\r'
        },
        None => {
            //this function is internal in-module use only
            unreachable!(
                "[BUG] this function is meant to be called with offset pointing to a character")
        }
    }

    for ch in chars {
        if hit_cr {
            buff.push('\n');
            hit_cr = ch == '\r';
            if ch != '\n' {
                buff.push(ch);
            }
        } else {
            if ch == '\n' {
                buff.push('\r')
            } else {
                hit_cr = ch == '\r'
            }
            buff.push(ch)
        }
    }

    if hit_cr {
        buff.push('\n')
    }

    buff
}


#[cfg(test)]
mod test {
    mod fix_newlines {
        use super::super::fix_newlines;

        #[test]
        fn replace_orphan_cr_nl() {
            assert_eq!(fix_newlines("abc\rdef\nghi".to_owned()), "abc\r\ndef\r\nghi");
            assert_eq!(fix_newlines("\rabc\r".to_owned()), "\r\nabc\r\n");
            assert_eq!(fix_newlines("\r".to_owned()), "\r\n");
            assert_eq!(fix_newlines("\nabc\n".to_owned()), "\r\nabc\r\n");
            assert_eq!(fix_newlines("\n".to_owned()), "\r\n");
            assert_eq!(fix_newlines("abc\nd".to_owned()), "abc\r\nd");
        }

        #[test]
        fn handle_multiple_orphan_cr_nl_in_row() {
            assert_eq!(fix_newlines("\r\r".to_owned()), "\r\n\r\n");
            assert_eq!(fix_newlines("\n\n".to_owned()), "\r\n\r\n");
            assert_eq!(fix_newlines("\r\r\n\n".to_owned()), "\r\n\r\n\r\n");
            assert_eq!(fix_newlines("\r\r\n\r".to_owned()), "\r\n\r\n\r\n");
        }

    }
    mod sniff_media_type {
        use std::path::Path;
        use ::error::CreatingSpecErrorVariant;
        use super::super::sniff_media_type;

        #[test]
        fn sniff_pdf() {
            let mt = sniff_media_type(Path::new("./test_resources/simple.pdf")).unwrap();
            assert_eq!(mt.as_str_repr(), "application/pdf; charset=binary")
        }

        #[test]
        fn sniff_image() {
            let mt = sniff_media_type(Path::new("./test_resources/png_image.png")).unwrap();
            assert_eq!(mt.as_str_repr(), "image/png; charset=binary")
        }

        #[test]
        fn sniff_ascii() {
            let mt = sniff_media_type(Path::new("./test_resources/ascii_text.txt")).unwrap();
            assert_eq!(mt.as_str_repr(), "text/plain; charset=us-ascii")
        }

        #[test]
        fn sniff_utf8() {
            let mt = sniff_media_type(Path::new("./test_resources/utf8_text.txt")).unwrap();
            assert_eq!(mt.as_str_repr(), "text/plain; charset=utf-8")
        }

        #[test]
        fn sniff_conflicting_image() {
            let _path = Path::new("./test_resources/jpg_image.png");
            let err = sniff_media_type(_path).unwrap_err();
            if let &CreatingSpecErrorVariant
                ::FileStemAndContentDifferInMediaType { ref path, ref by_extension, ref by_content }
                = err.variant()
            {
                assert_eq!(path, _path);
                assert_eq!(by_extension, "image/png");
                assert_eq!(by_content, "image/jpeg");
            } else {
                panic!("unexpected error: {}", err);
            }
        }
    }
}