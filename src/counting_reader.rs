use acprotocol::dat::reader::range_reader::RangeReader;

pub struct CountingRangeReader<R> {
    pub inner: R,
    pub count: usize,
}

impl<R> CountingRangeReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, count: 0 }
    }
}

impl<R: RangeReader> RangeReader for CountingRangeReader<R> {
    fn read_range(
        &mut self,
        offset: u32,
        length: usize,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, Box<dyn std::error::Error>>> {
        self.count += 1;
        self.inner.read_range(offset, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockReader;

    impl RangeReader for MockReader {
        fn read_range(
            &mut self,
            _offset: u32,
            length: usize,
        ) -> impl std::future::Future<Output = Result<Vec<u8>, Box<dyn std::error::Error>>> {
            async move { Ok(vec![0u8; length]) }
        }
    }

    #[tokio::test]
    async fn test_count_starts_at_zero() {
        let reader = CountingRangeReader::new(MockReader);
        assert_eq!(reader.count, 0);
    }

    #[tokio::test]
    async fn test_count_increments_per_call() {
        let mut reader = CountingRangeReader::new(MockReader);
        reader.read_range(0, 1024).await.unwrap();
        assert_eq!(reader.count, 1);
        reader.read_range(1024, 1024).await.unwrap();
        assert_eq!(reader.count, 2);
        reader.read_range(2048, 512).await.unwrap();
        assert_eq!(reader.count, 3);
    }

    #[tokio::test]
    async fn test_delegates_data_to_inner_reader() {
        let mut reader = CountingRangeReader::new(MockReader);
        let data = reader.read_range(0, 42).await.unwrap();
        assert_eq!(data.len(), 42);
        assert!(data.iter().all(|&b| b == 0));
    }
}
