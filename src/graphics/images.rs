use quicksilver::graphics::Image;
use quicksilver::lifecycle::Asset;
use std::collections::HashMap;
use std::fs;

pub type ImageMap = HashMap<String, Asset<Image>>;

pub fn fetch_images() -> ImageMap {
    fs::read_dir("./")
        .expect("Couldn't find `./static` folder!")
        .filter_map(|img| {
            let path = img.ok()?.path();
            Some((
                path.file_stem()?.to_string_lossy().to_string(),
                Asset::new(Image::load(path.file_name()?.to_string_lossy().to_string())),
            ))
        })
        .collect()
}
