#[allow(clippy::cast_sign_loss)]
pub fn mask_user(user: i64) -> String {
    format!("{:x}", md5::compute((user as u128).to_le_bytes()))
}
