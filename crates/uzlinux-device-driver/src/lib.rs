pub mod event;
pub mod traits;
pub mod virtual_deck;
pub mod manager;
pub mod ulanzi;

pub use event::DeviceEvent;
pub use traits::StreamDeckDevice;
pub use virtual_deck::VirtualDeck;
pub use manager::DeviceManager;
pub use ulanzi::UlanziD200Device;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_zip_writer_options() {
        use std::io::{Cursor, Write};
        use rand::Rng;
        use rand::seq::SliceRandom;

        let mut numbers: Vec<usize> = (0..14).collect();
        numbers.shuffle(&mut rand::thread_rng());
        let dummy_content: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(100)
            .map(char::from)
            .collect();

        let mut cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("test.txt", options).unwrap();
        zip.write_all(dummy_content.as_bytes()).unwrap();
        zip.finish().unwrap();
        let data = cursor.into_inner();
        assert!(!data.is_empty());
    }
}
