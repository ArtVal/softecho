//! Встроенные картинки для тематических наборов (офлайн).

/// PNG-байты по id (`cup`, `house`, …).
pub fn image_png(id: &str) -> Option<&'static [u8]> {
    match id {
        "cup" => Some(include_bytes!("../../assets/images/cup.png")),
        "house" => Some(include_bytes!("../../assets/images/house.png")),
        "apple" => Some(include_bytes!("../../assets/images/apple.png")),
        "cat" => Some(include_bytes!("../../assets/images/cat.png")),
        "water" => Some(include_bytes!("../../assets/images/water.png")),
        "bread" => Some(include_bytes!("../../assets/images/bread.png")),
        _ => None,
    }
}

pub fn decode_rgba(id: &str) -> Option<(usize, usize, Vec<u8>)> {
    let bytes = image_png(id)?;
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some((w as usize, h as usize, img.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_images_decode() {
        for id in ["cup", "house", "apple", "cat", "water", "bread"] {
            let (w, h, rgba) = decode_rgba(id).unwrap_or_else(|| panic!("{id}"));
            assert!(w >= 32 && h >= 32, "{id}: {w}x{h}");
            assert_eq!(rgba.len(), w * h * 4);
        }
        assert!(decode_rgba("nope").is_none());
    }
}
