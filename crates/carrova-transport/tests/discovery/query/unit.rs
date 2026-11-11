use carrova_crypto::pairing::{Nameplate, ResumeSecret};

use super::*;
use crate::{BROADCAST_BROWSE, BROADCAST_QUERY, TXT_DEVICE_NAME};

#[test]
fn broadcast_reply_parses_port_only_with_prefix() {
    let mut r = BROADCAST_REPLY.to_vec();
    r.extend_from_slice(b"40123");
    assert_eq!(parse_reply(&r), Some((40123, None)));
    assert_eq!(parse_reply(b"40123"), None);
    let mut bad = BROADCAST_REPLY.to_vec();
    bad.extend_from_slice(b"70000");
    assert_eq!(parse_reply(&bad), None);
}

#[test]
fn browse_reply_carries_the_device_name_after_the_port() {
    let named = [BROADCAST_REPLY, b"40123 ezio's laptop"].concat();
    assert_eq!(
        parse_reply(&named),
        Some((40123, Some("ezio's laptop".to_owned())))
    );
    let not_utf8 = [BROADCAST_REPLY, b"40123 \xff"].concat();
    assert_eq!(parse_reply(&not_utf8), None);
}

fn pairing_offer() -> Offer {
    Offer::new(Subject::Pair(Nameplate::new(7).unwrap())).unwrap()
}

#[test]
fn senders_name_themselves_only_to_browsers_and_only_while_offering_a_code() {
    let np_query = [BROADCAST_QUERY, b"np=7"].concat();
    let pairing = pairing_offer();
    assert_eq!(
        reply(&pairing, &np_query, 40123, Some("box")),
        Some([BROADCAST_REPLY, b"40123"].concat()),
        "pairing replies stay byte-identical for older receivers"
    );
    assert_eq!(
        reply(&pairing, BROADCAST_BROWSE, 40123, Some("box")),
        Some([BROADCAST_REPLY, b"40123 box"].concat())
    );
    assert_eq!(reply(&pairing, BROADCAST_BROWSE, 40123, None), None);
    let resume = Offer::new(Subject::Resume(ResumeSecret::from_bytes([1; 32]))).unwrap();
    assert_eq!(reply(&resume, BROADCAST_BROWSE, 40123, Some("box")), None);
    assert_eq!(BROADCAST_BROWSE, b"CARROVA/1?browse");
}

#[test]
fn advertised_names_fit_one_txt_entry_on_a_char_boundary() {
    assert_eq!(txt_fit("laptop"), "laptop");
    let long = "é".repeat(200);
    let fitted = txt_fit(&long);
    assert!(TXT_DEVICE_NAME.len() + 1 + fitted.len() <= usize::from(u8::MAX));
    assert!(long.starts_with(fitted) && !fitted.is_empty());
}
