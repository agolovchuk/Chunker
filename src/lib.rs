#[derive(Default)]
pub struct Chunk<'a> {
    counter: usize,
    topic: u8,
    data: &'a [u8],
    max_chunk_size: usize,
    meta_size: usize,
    pub status: ChunkStatus,
}

#[derive(Debug)]
pub enum ChunkSessionStatus {
    Sended,
    Received,
}
#[derive(Default, Debug)]
pub struct ChunkStatus {
    pub number: Option<usize>,
    pub session: Option<ChunkSessionStatus>,
    pub retry: u8,
}

#[derive(Debug)]
pub enum ChunkError {
    InvalidMetaSize,
    OverflowRetryCounter,
}

impl ChunkStatus {
    pub fn new() -> Self {
        ChunkStatus {
            ..Default::default()
        }
    }

    pub fn to_send(&mut self, number: usize) {
        self.number = Some(number);
        self.session = Some(ChunkSessionStatus::Sended);
        self.retry = 0;
    }

    pub fn to_received(&mut self, number: usize) {
        if self.number != Some(number) {
            panic!(
                "Invalid chunk number, current {}, received {}",
                self.number.unwrap(),
                number
            );
        }
        self.session = Some(ChunkSessionStatus::Received);
        self.retry = 0;
    }

    pub fn increase_retry(&mut self) -> Result<u8, ChunkError> {
        println!("<---Counter increase: {}", self.retry);
        if self.retry == u8::MAX {
            return Err(ChunkError::OverflowRetryCounter);
        }
        self.retry += 1;
        Ok(self.retry)
    }
}

impl<'a> Chunk<'a> {
    pub fn new(max_chunk_size: usize, topic: u8, data: &'a [u8]) -> Self {
        Chunk {
            counter: 0,
            data,
            topic,
            max_chunk_size,
            meta_size: core::mem::size_of::<usize>(),
            status: ChunkStatus::new(),
        }
    }

    /**
     * header must contain length of data and topic
     */
    pub fn header(&self) -> [u8; core::mem::size_of::<usize>() + 1] {
        let len = self.data.len();
        let mut header = [0; core::mem::size_of::<usize>() + 1];
        header[..1].copy_from_slice(&self.topic.to_le_bytes());
        header[1..].copy_from_slice(&len.to_le_bytes());
        header
    }


    pub fn counter(&self) -> usize {
        self.counter
    }

    pub fn meta(resp: &[u8]) -> Result<usize, ChunkError> {
        if resp.len() < core::mem::size_of::<usize>() {
            return Err(ChunkError::InvalidMetaSize);
        }
        let meta: &[u8; core::mem::size_of::<usize>()] =
            &resp[..core::mem::size_of::<usize>()].try_into().unwrap();
        Ok(usize::from_le_bytes(*meta))
    }

    fn inc_counter(&mut self) {
        self.counter += 1;
    }

    fn get_pointer(&self, counter: usize) -> usize {
        /*
         * 0 iter = 0 * 250 - 4 * 0 = 0
         * 1 iter = 1 * 250 - 4 * 1 - 8 = 238
         * 2 iter = 2 * 250 - 4 * 2 - 8 = 484
         */
        if counter == 0 {
            return 0;
        }
        counter * self.max_chunk_size - self.meta_size * counter - self.header().len()
    }

    fn start(&self, counter: Option<usize>) -> usize {
        let counter = counter.unwrap_or(self.counter);
        self.get_pointer(counter)
    }

    fn end(&self, counter: Option<usize>) -> usize {
        self.get_pointer(counter.unwrap_or(self.counter) + 1)
    }

    pub fn chunk(&self, counter: Option<usize>) -> Option<(&'a [u8], usize)> {
        let start = self.start(counter);
        if start > self.data.len() {
            return None;
        }
        let end = self.end(counter);
        if end > self.data.len() {
            return Some((&self.data[start..], counter.unwrap_or(self.counter)));
        }
        Some((&self.data[start..end], counter.unwrap_or(self.counter)))
    }
}

impl<'a> Iterator for Chunk<'a> {
    type Item = (&'a [u8], usize);

    fn next(&mut self) -> Option<Self::Item> {
        match self.chunk(Some(self.counter)) {
            Some(chunk) => {
                self.inc_counter();
                Some(chunk)
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk() {
        let data = vec![0; 1000];
        let chunk = Chunk::new(250, 0x10, &data);
        let mut iter = chunk.into_iter();
        assert_eq!(iter.next().unwrap().0.len() + core::mem::size_of::<usize>() * 2 + 1, 250);
        assert_eq!(iter.next().unwrap().0.len() + core::mem::size_of::<usize>(), 250);
        assert_eq!(iter.next().unwrap().0.len() + core::mem::size_of::<usize>(), 250);
    }

    #[test]
    fn test_header() {
        let data = vec![0; 1000];
        let chunk = Chunk::new(250, 0x10, &data);
        let header = chunk.header();

        assert_eq!(header.len(), core::mem::size_of::<usize>() + 1);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(header, [0x10, 0xE8, 0x03, 0, 0, 0, 0, 0, 0]);
    }
}
