#[cfg(test)]
mod tests {
    use super::ContentDisposition;

    #[test]
    fn parses_rfc5987_filename() {
        let disposition = ContentDisposition::parse(
            "attachment; filename=\"file_name.jpg\"; filename*=UTF-8''file%20name.jpg",
        )
        .unwrap();

        assert!(disposition.is_attachment());
        assert_eq!(disposition.filename(), Some("file name.jpg"));
        assert_eq!(disposition.ascii_filename(), Some("file_name.jpg"));
        assert_eq!(
            disposition.rfc5987_filename(),
            Some("UTF-8''file%20name.jpg")
        );
    }

    #[test]
    fn creates_unicode_attachment() {
        let disposition = ContentDisposition::attachment("fïle nãme.jpg");

        assert_eq!(disposition.filename(), Some("fïle nãme.jpg"));
        assert_eq!(disposition.ascii_filename(), Some("file name.jpg"));
        assert_eq!(
            disposition.value(),
            "attachment; filename=\"file name.jpg\"; filename*=UTF-8''f%C3%AFle%20n%C3%A3me.jpg"
        );
    }
}
