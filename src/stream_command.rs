use bytevec2::{ByteDecodable, ByteEncodable};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StreamCommandError {
    #[error("Serialization/Deserialization error: {0}")]
    SerializationError(#[from] bytevec2::errors::ByteVecError),
    #[error("Parser error")]
    ParserError,
}

pub struct StreamCommand {
    limit: usize,
    remainder: Vec<u8>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ParseStatus {
    Begin,
    Heading,
    Text,
    Data,
    Ended,
}

impl StreamCommand {
    pub fn new(limit: usize) -> Self {
        let remainder = Vec::<u8>::with_capacity(limit);

        Self { limit, remainder }
    }

    pub fn reset(&mut self) {
        self.remainder.clear();
    }

    /// Encode a command to be sent over a stream
    pub fn encode<T>(&self, value: T) -> Result<Vec<u8>, StreamCommandError>
    where
        T: ByteEncodable,
    {
        let mut result = vec![];

        let mut crc = 0x00;

        result.push(0x01u8);

        result.push(0x02u8);

        for d in value
            .encode::<u32>()
            .map_err(|err| StreamCommandError::SerializationError(err))?
        {
            if (d == 0x01u8) || (d == 0x02u8) || (d == 0x03u8) || (d == 0x04u8) || (d == 0x20u8) {
                result.push(0x20u8);
            }

            result.push(d);
            crc = crc ^ d;
        }

        result.push(0x03u8);

        result.push(crc);

        result.push(0x04u8);

        Ok(result)
    }

    pub fn decode<T>(&mut self, data: &[u8]) -> Result<Vec<T>, StreamCommandError>
    where
        T: ByteDecodable,
    {
        // small optimization: if no data is provided then nothing can come out of this function
        if data.is_empty() {
            return Ok(vec![])
        }

        while (self.remainder.len() > 0) && (self.remainder[0] != 0x01u8) {
            self.remainder.remove(0);
        }

        for d in data {
            if (self.limit - 1) == self.remainder.len() {
                self.remainder.remove(0);
            }

            self.remainder.push(*d);
        }

        let mut decoded: Vec<T> = vec![];

        'decode: loop {
            let mut found = false;
            'check: for d in self.remainder.iter() {
                if *d == 0x04u8 {
                    found = true;
                    break 'check;
                }
            }

            if !found {
                break 'decode;
            }

            let mut data: Vec<u8> = vec![];
            let mut escape = false;
            let mut parsing = ParseStatus::Begin;
            let mut crc: Option<u8> = None;

            'parser: while !self.remainder.is_empty() {
                let d = self.remainder.remove(0);
                match parsing {
                    ParseStatus::Begin => match d {
                        0x01u8 => parsing = ParseStatus::Heading,
                        _ => return Err(StreamCommandError::ParserError),
                    },
                    ParseStatus::Heading => match d {
                        0x02u8 => parsing = ParseStatus::Text,
                        _ => return Err(StreamCommandError::ParserError),
                    },
                    ParseStatus::Text => {
                        let do_escape = escape;
                        escape = false;

                        match do_escape {
                            true => data.push(d),
                            false => match d {
                                0x01u8 => return Err(StreamCommandError::ParserError),
                                0x02u8 => return Err(StreamCommandError::ParserError),
                                0x03u8 => parsing = ParseStatus::Data,
                                0x04u8 => return Err(StreamCommandError::ParserError),
                                0x20u8 => escape = true,
                                _ => data.push(d),
                            },
                        }
                    }
                    ParseStatus::Data => {
                        if crc.is_none() {
                            crc = Some(d);
                        } else if d == 0x04u8 {
                            parsing = ParseStatus::Ended;
                            break 'parser;
                        } else {
                            return Err(StreamCommandError::ParserError);
                        }
                    }
                    ParseStatus::Ended => return Err(StreamCommandError::ParserError),
                }
            }

            if crc.is_none() {
                return Err(StreamCommandError::ParserError);
            }

            if parsing != ParseStatus::Ended {
                return Err(StreamCommandError::ParserError);
            }

            decoded.push(
                T::decode::<u32>(&data)
                    .map_err(|err| StreamCommandError::SerializationError(err))?,
            );
        }

        Ok(decoded)
    }
}

impl Default for StreamCommand {
    fn default() -> Self {
        Self::new(1024)
    }
}
