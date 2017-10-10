use std::io;
use std::ptr;
use std::marker::PhantomData;
use ::Result;
use serde_crate as serde;

/// A byte-oriented reading trait that is specialized for
/// slices and generic readers.
pub trait BincodeRead<'storage>: io::Read + ::private::Sealed {
    #[doc(hidden)]
    fn forward_read_str<V>(&mut self, length: usize, visitor: V) ->  Result<V::Value>
    where V: serde::de::Visitor<'storage>;

    #[doc(hidden)]
    fn get_byte_buffer(&mut self, length: usize) -> Result<Vec<u8>>;

    #[doc(hidden)]
    fn forward_read_bytes<V>(&mut self, length: usize, visitor: V) ->  Result<V::Value>
    where V: serde::de::Visitor<'storage>;
}

/// A BincodeRead implementation for byte slices
pub struct SliceReader<'storage> {
    storage: PhantomData<&'storage u8>,
    buf: *const u8,
    end: *const u8
}

/// A BincodeRead implementation for io::Readers
pub struct IoReader<R> {
    reader: R,
    temp_buffer: Vec<u8>,
}

impl <'storage> SliceReader<'storage> {
    /// Constructs a slice reader
    pub fn new(bytes: &'storage [u8]) -> SliceReader<'storage> {
        unsafe {
            let end = bytes.as_ptr().offset(bytes.len() as isize);
            let buf = bytes.as_ptr();
            SliceReader {storage: PhantomData, buf, end}
        }
    }
}

impl <R> IoReader<R> {
    /// Constructs an IoReadReader
    pub fn new(r: R) -> IoReader<R> {
        IoReader {
            reader: r,
            temp_buffer: vec![],
        }
    }
}

impl <'storage> io::Read for SliceReader<'storage> {
    fn read(&mut self, out: & mut [u8]) -> io::Result<usize> {
        unsafe {
            if self.buf.offset(out.len() as isize) > self.end {
                return Err(io::ErrorKind::UnexpectedEof.into())
            }
            ptr::copy_nonoverlapping(self.buf, out.as_mut_ptr(), out.len());
            self.buf = self.buf.offset(out.len() as isize);
        }
        Ok(out.len())
    }
}

impl <R: io::Read> io::Read for IoReader<R> {
    fn read(&mut self, out: & mut [u8]) -> io::Result<usize> {
        self.reader.read(out)
    }
}

impl <R> IoReader<R> where R: io::Read {
    fn fill_buffer(&mut self, length: usize) -> Result<()> {
        let current_length = self.temp_buffer.len();
        if length > current_length{
            self.temp_buffer.reserve_exact(length - current_length);
            unsafe { self.temp_buffer.set_len(length); }
        }

        self.reader.read_exact(&mut self.temp_buffer[..length])?;
        Ok(())
    }
}

impl <R> BincodeRead<'static> for IoReader<R> where R: io::Read {
    fn forward_read_str<V>(&mut self, length: usize, visitor: V) ->  Result<V::Value>
    where V: serde::de::Visitor<'static> {
        self.fill_buffer(length)?;

        let string = match ::std::str::from_utf8(&self.temp_buffer[..length]) {
            Ok(s) => s,
            Err(_) => return Err(Box::new(::ErrorKind::InvalidEncoding {
                desc: "string was not valid utf8",
                detail: None,
            })),
        };

        let r = visitor.visit_str(string);
        r
    }

    fn get_byte_buffer(&mut self, length: usize) -> Result<Vec<u8>> {
        self.fill_buffer(length)?;
        Ok(self.temp_buffer[..length].to_vec())
    }

    fn forward_read_bytes<V>(&mut self, length: usize, visitor: V) ->  Result<V::Value>
    where V: serde::de::Visitor<'static> {
        self.fill_buffer(length)?;
        let r = visitor.visit_bytes(&self.temp_buffer[..length]);
        r
    }
}
