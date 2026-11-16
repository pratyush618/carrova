use super::looks_like_code;

#[test]
fn otp_detected() {
    assert!(looks_like_code("123456 is your verification code"));
    assert!(looks_like_code("Your OTP: 4821"));
    assert!(looks_like_code("Use 12345678 to Sign-In"));
}

#[test]
fn otp_not_detected_in_prices() {
    assert!(!looks_like_code("Your order total 1299 rupees"));
    assert!(!looks_like_code("Your code is ready for pickup"));
    assert!(!looks_like_code("code 123 and 123456789"));
    let far = format!("code{}1234", " ".repeat(40));
    assert!(!looks_like_code(&far));
}

#[test]
fn code_words_need_word_boundaries() {
    assert!(!looks_like_code("Shopping total 1299"));
    assert!(!looks_like_code("Shipping update: order 12345678"));
    assert!(!looks_like_code("Your barcode 12345678"));
    assert!(looks_like_code("Your security code is 123456"));
    assert!(looks_like_code("(code)123456"));
}
