use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Reads the entire content of a file into a byte vector.
///
/// # Arguments
/// * `jar_file_path` - The path to the file to read.
///
/// # Returns
/// A `Result` which is `Ok(Vec<u8>)` on success, or `Err(std::io::Error)` if an error occurs
/// during file opening or reading.
pub fn get_jar_contents(jar_file_path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(jar_file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_jar_contents() {
        let content = b"This is some test content.";
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content)
            .expect("Failed to write to temp file");
        let path = temp_file.path();

        let read_content = get_jar_contents(path).expect("Failed to read jar contents");
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_get_jar_contents_non_existent_file() {
        let path = Path::new("non_existent_file.jar");
        let result = get_jar_contents(path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }
}
