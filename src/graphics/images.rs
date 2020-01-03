use fxhash::FxHashMap;
use quicksilver::graphics::Image;
use quicksilver::lifecycle::Asset;
use std::fs;

pub type ImageMap = FxHashMap<String, Asset<Image>>;

pub fn fetch_images() -> ImageMap {
    fs::read_dir("./img/")
        .expect("Couldn't find `./static` folder!")
        .filter_map(|img| {
            let path = img.ok()?.path();
            Some((
                path.file_stem()?.to_string_lossy().to_string(),
                Asset::new(Image::load(format!(
                    "./img/{}",
                    path.file_name()?.to_string_lossy().to_string()
                ))),
            ))
        })
        .collect()
}
