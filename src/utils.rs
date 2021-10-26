pub fn mask_user(user: i64) -> String {
    format!("{:x}", md5::compute(user.to_le_bytes()))
}