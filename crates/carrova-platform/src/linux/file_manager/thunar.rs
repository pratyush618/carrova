//! Our custom action inside Thunar's `uca.xml`, edited at string level.
//!
//! No XML dependency: we only cut the `<action>` that holds our unique id
//! and insert ours before the last `</actions>`, so nothing outside our
//! block changes. Tags are only searched for outside comments and CDATA
//! sections. Anything we cannot place that way is [`Malformed`].

const ID_TAG: &str = "<unique-id>carrova-send-with</unique-id>";
const OPEN: &str = "<action>";
const CLOSE: &str = "</action>";
const ROOT_OPEN: &str = "<actions>";
const ROOT_CLOSE: &str = "</actions>";
/// Text no tag search may match inside: comments and CDATA sections.
const OPAQUE: [(&str, &str); 2] = [("<!--", "-->"), ("<![CDATA[", "]]>")];

/// `uca.xml` is not a layout we can edit without risking the user's actions.
#[derive(Debug)]
pub(super) struct Malformed;

/// A fresh `uca.xml` holding only `action`.
pub(super) fn new_file(action: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{ROOT_OPEN}\n{action}{ROOT_CLOSE}\n")
}

/// Whether `text` carries our action.
pub(super) fn has_action(text: &str) -> bool {
    uncommented(text).is_ok_and(|view| view.contains(ID_TAG))
}

/// `text` with our action (re)placed last; `action` ends with a newline.
pub(super) fn with_action(text: &str, action: &str) -> Result<String, Malformed> {
    let mut text = without_action(text)?;
    let at = root_close(&uncommented(&text)?)?;
    let lead = if text[..at].ends_with('\n') { "" } else { "\n" };
    text.insert_str(at, &format!("{lead}{action}"));
    Ok(text)
}

/// `text` with every copy of our action cut out, indentation and trailing
/// newline included. Tags are found in the [`uncommented`] view, whose
/// offsets match `text`, and both are cut alike.
pub(super) fn without_action(text: &str) -> Result<String, Malformed> {
    let mut view = uncommented(text)?;
    root_close(&view)?;
    let mut text = text.to_owned();
    while let Some(id) = view.find(ID_TAG) {
        let start = view[..id].rfind(OPEN).ok_or(Malformed)?;
        if view[start..id].contains(CLOSE) {
            return Err(Malformed);
        }
        let close = id + view[id..].find(CLOSE).ok_or(Malformed)?;
        // Another action opening first means ours never closed: cutting to
        // `close` would take the user's action with ours.
        if view[id..close].contains(OPEN) {
            return Err(Malformed);
        }
        let end = close + CLOSE.len();
        let start = text[..start].trim_end_matches([' ', '\t']).len();
        let end = end + usize::from(text[end..].starts_with('\n'));
        text.replace_range(start..end, "");
        view.replace_range(start..end, "");
    }
    Ok(text)
}

/// `text` with every comment and CDATA section ([`OPAQUE`]) blanked to
/// spaces, byte offsets kept, so no search matches inside one. One that
/// never ends is [`Malformed`].
fn uncommented(text: &str) -> Result<String, Malformed> {
    let mut view = text.to_owned();
    let mut from = 0;
    // The earliest opener wins: a comment may hold `<![CDATA[` and vice versa.
    while let Some((open, (start, close))) = OPAQUE
        .iter()
        .filter_map(|&pair| Some((from + view[from..].find(pair.0)?, pair)))
        .min_by_key(|(at, _)| *at)
    {
        let body = open + start.len();
        let end = body + view[body..].find(close).ok_or(Malformed)? + close.len();
        view.replace_range(open..end, &" ".repeat(end - open));
        from = end;
    }
    Ok(view)
}

/// Byte offset of the closing root tag, which must follow an opening one.
fn root_close(text: &str) -> Result<usize, Malformed> {
    let close = text.rfind(ROOT_CLOSE).ok_or(Malformed)?;
    if text[..close].contains(ROOT_OPEN) {
        Ok(close)
    } else {
        Err(Malformed)
    }
}
