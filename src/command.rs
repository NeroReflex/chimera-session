use std::ffi::OsString;

use bytevec2::*;

bytevec_decl! {
    #[derive(PartialEq, Eq, Debug, Clone)]
    pub struct SessionExecutable {
        program: String,
        arguments: Vec<String>
    }
}

impl SessionExecutable {
    pub fn new(name: &str) -> Self {
        match name {
            "" => Self {
                program: String::from("/bin/sh"),
                arguments: vec![],
            },
            "bash" => Self {
                program: String::from("/bin/bash"),
                arguments: vec![],
            },
            "zsh" => Self {
                program: String::from("/bin/zsh"),
                arguments: vec![],
            },
            "sleep" => Self {
                program: String::from("sleep"),
                arguments: vec![String::from("5")],
            },
            _ => todo!(),
        }
    }

    pub fn get_program(&self) -> OsString {
        OsString::from(self.program.as_str())
    }

    pub fn get_arguments(&self) -> Vec<OsString> {
        self.arguments
            .iter()
            .map(|a| OsString::from(a.as_str()))
            .collect::<Vec<OsString>>()
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum ChimeraSessionCommand {
    Terminate,
    Restart(SessionExecutable),
}

impl ByteEncodable for ChimeraSessionCommand {
    fn get_size<Size>(&self) -> Option<Size>
    where
        Size: BVSize + ByteEncodable,
    {
        let min_size = BVSize::from_usize(1);

        match self {
            Self::Terminate => Some(min_size),
            Self::Restart(exec) => match exec.get_size() {
                Some(contained_size) => Some(min_size.checked_add(contained_size)?),
                None => Some(min_size),
            },
        }
    }

    fn encode<Size>(&self) -> BVEncodeResult<Vec<u8>>
    where
        Size: BVSize + ByteEncodable,
    {
        match self {
            Self::Terminate => Ok(vec![0x00u8]),
            Self::Restart(exec) => Ok(vec![0x01u8]
                .iter()
                .chain(exec.encode::<Size>()?.iter())
                .cloned()
                .collect()),
        }
    }
}

impl ByteDecodable for ChimeraSessionCommand {
    fn decode<Size>(bytes: &[u8]) -> BVDecodeResult<Self>
    where
        Size: BVSize + ByteDecodable,
    {
        match bytes.len() {
            0 => Err(bytevec2::errors::ByteVecError::BadSizeDecodeError {
                expected: bytevec2::errors::BVExpectedSize::MoreThan(0),
                actual: 0,
            }),
            _ => match bytes[0] {
                0x00u8 => Ok(ChimeraSessionCommand::Terminate),
                0x01u8 => Ok(ChimeraSessionCommand::Restart(SessionExecutable::decode::<
                    Size,
                >(
                    &bytes[1..]
                )?)),
                _ => unreachable!(),
            },
        }
    }
}
