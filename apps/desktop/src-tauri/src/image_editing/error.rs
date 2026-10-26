use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEditingError {
    pub code: String,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, ImageEditingError>;

impl ImageEditingError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }

    pub fn unsupported_format() -> Self {
        Self::new("unsupported_image_format", "Only non-animated RGB or RGBA PNG source images are accepted by the current import boundary.")
    }

    pub fn invalid_image() -> Self {
        Self::new("invalid_image", "The selected source is not a valid, fully decodable PNG image.")
    }

    pub fn image_too_large() -> Self {
        Self::new("image_too_large", "The source image exceeds the 8 MiB import limit.")
    }

    pub fn dimensions() -> Self {
        Self::new("image_dimensions", "The source image must be at most 2,048 by 2,048 pixels and no more than 4 megapixels.")
    }

    pub fn insufficient_storage() -> Self {
        Self::new("insufficient_storage", "Free space for the application-managed source image could not be confirmed or is insufficient.")
    }

    pub fn unknown_source() -> Self {
        Self::new("unknown_image_source", "That source image is not a valid image-editing source managed by Nexora.")
    }

    pub fn integrity() -> Self {
        Self::new("integrity_failed", "The managed source image no longer matches its recorded dimensions or SHA-256 digest.")
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new("storage", message)
    }
}
