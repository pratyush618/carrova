//! Prints this computer's call-audio state (read-only):
//! `cargo run --example call_audio_status -p carrova-platform`.

fn main() {
    match carrova_platform::current().call_audio_status() {
        Ok(status) => println!("{status:?}"),
        Err(e) => println!("error: {e}"),
    }
}
