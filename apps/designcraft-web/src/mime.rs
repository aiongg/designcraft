//! The MIME type of a file the browser downloads.

/// The MIME type for a download named `name`, by its extension.
pub fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("pdf") => "application/pdf",
        Some("designcraft") => designcraft_format::MIME,
        Some("idml") => "application/vnd.adobe.indesign-idml-package",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::mime_for;

    #[test]
    fn a_native_document_downloads_as_its_zip_package() {
        assert_eq!(mime_for("Spring Issue.designcraft"), designcraft_format::MIME);
        assert_eq!(mime_for("a.b.DesignCraft"), "application/vnd.designcraft+zip");
        assert_eq!(mime_for("cover.png"), "image/png");
        assert_eq!(mime_for("issue.pdf"), "application/pdf");
        assert_eq!(mime_for("noextension"), "application/octet-stream");
    }
}
