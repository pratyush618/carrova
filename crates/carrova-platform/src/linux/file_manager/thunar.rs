//! Our custom action inside Thunar's `uca.xml`, edited at string level.
//!
//! No XML dependency: we only cut the `<action>` that holds our unique id
//! and insert ours before the last `</actions>`, so nothing outside our
//! block changes. Anything we cannot place that way is [`Malformed`].

const ID_TAG: &str = "<unique-id>carrova-send-with</unique-id>";
const OPEN: &str = "<action>";
const CLOSE: &str = "</action>";
const ROOT_OPEN: &str = "<actions>";
const ROOT_CLOSE: &str = "</actions>";

/// `uca.xml` is not a layout we can edit without risking the user's actions.
#[derive(Debug)]
pub(super) struct Malformed;

/// A fresh `uca.xml` holding only `action`.
pub(super) fn new_file(action: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{ROOT_OPEN}\n{action}{ROOT_CLOSE}\n")
}

/// Whether `text` carries our action.
pub(super) fn has_action(text: &str) -> bool {
    text.contains(ID_TAG)
}

/// `text` with our action (re)placed last; `action` ends with a newline.
pub(super) fn with_action(text: &str, action: &str) -> Result<String, Malformed> {
    let mut text = without_action(text)?;
    let at = root_close(&text)?;
    let lead = if text[..at].ends_with('\n') { "" } else { "\n" };
    text.insert_str(at, &format!("{lead}{action}"));
    Ok(text)
}

/// `text` with every copy of our action cut out, indentation and trailing
/// newline included.
pub(super) fn without_action(text: &str) -> Result<String, Malformed> {
    root_close(text)?;
    let mut text = text.to_owned();
    while let Some(id) = text.find(ID_TAG) {
        let start = text[..id].rfind(OPEN).ok_or(Malformed)?;
        if text[start..id].contains(CLOSE) {
            return Err(Malformed);
        }
        let close = id + text[id..].find(CLOSE).ok_or(Malformed)?;
        // Another action opening first means ours never closed: cutting to
        // `close` would take the user's action with ours.
        if text[id..close].contains(OPEN) {
            return Err(Malformed);
        }
        let end = close + CLOSE.len();
        let start = text[..start].trim_end_matches([' ', '\t']).len();
        let end = end + usize::from(text[end..].starts_with('\n'));
        text.replace_range(start..end, "");
    }
    Ok(text)
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
