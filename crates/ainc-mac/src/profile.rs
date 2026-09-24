//! Read the local macOS account at launch. The photo never leaves the device.
use gpui::{Image, ImageFormat};
use std::{process::Command, sync::Arc};
pub struct Profile {
    pub name: String,
    pub photo: Option<Arc<Image>>,
}
impl Profile {
    pub fn local() -> Self {
        let name = Command::new("/usr/bin/id")
            .arg("-F")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|name| name.split_whitespace().next().map(str::to_owned))
            .unwrap_or_else(|| "Profile".into());
        let photo = std::env::var("USER").ok().and_then(|user| {
            let output = Command::new("/usr/bin/dscl")
                .args([".", "-read", &format!("/Users/{user}"), "JPEGPhoto"])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let text = String::from_utf8(output.stdout).ok()?;
            let hex: Vec<u8> = text
                .split_once(':')?
                .1
                .bytes()
                .filter(u8::is_ascii_hexdigit)
                .collect();
            let bytes: Option<Vec<u8>> = hex
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    Some(
                        (char::from(pair[0]).to_digit(16)? * 16
                            + char::from(pair[1]).to_digit(16)?) as u8,
                    )
                })
                .collect();
            let bytes = bytes?;
            if !bytes.starts_with(&[0xff, 0xd8]) {
                return None;
            }
            Some(Arc::new(Image::from_bytes(ImageFormat::Jpeg, bytes)))
        });
        Self { name, photo }
    }
}
