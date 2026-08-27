//! Emit the canonical local inventory wire fixture for browser contract tests.
fn main() {
    println!("{:?}", playsrc_tf2::equipment::Equipment::default().encode_state());
}
