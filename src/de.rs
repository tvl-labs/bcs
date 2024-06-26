// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(feature = "std"))]
use alloc::{str, string::String, vec, vec::Vec};

#[cfg(feature = "std")]
use std::str;

use crate::error::{Error, Result};
use crate::io::Read;
use core::convert::TryFrom;
use serde::de::{self, Deserialize, DeserializeOwned, DeserializeSeed, IntoDeserializer, Visitor};

/// Deserializes a `&[u8]` into a type.
///
/// This function will attempt to interpret `bytes` as the BCS serialized form of `T` and
/// deserialize `T` from `bytes`.
///
/// # Examples
///
/// ```
/// use bcs::from_bytes;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Ip([u8; 4]);
///
/// #[derive(Deserialize)]
/// struct Port(u16);
///
/// #[derive(Deserialize)]
/// struct SocketAddr {
///     ip: Ip,
///     port: Port,
/// }
///
/// let bytes = vec![0x7f, 0x00, 0x00, 0x01, 0x41, 0x1f];
/// let socket_addr: SocketAddr = from_bytes(&bytes).unwrap();
///
/// assert_eq!(socket_addr.ip.0, [127, 0, 0, 1]);
/// assert_eq!(socket_addr.port.0, 8001);
/// ```
pub fn from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::new(bytes, crate::MAX_CONTAINER_DEPTH);
    let t = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Same as `from_bytes` but use `limit` as max container depth instead of MAX_CONTAINER_DEPTH`
/// Note that `limit` has to be lower than MAX_CONTAINER_DEPTH
pub fn from_bytes_with_limit<'a, T>(bytes: &'a [u8], limit: usize) -> Result<T>
where
    T: Deserialize<'a>,
{
    if limit > crate::MAX_CONTAINER_DEPTH {
        return Err(Error::NotSupported("limit exceeds the max allowed depth"));
    }
    let mut deserializer = Deserializer::new(bytes, limit);
    let t = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Perform a stateful deserialization from a `&[u8]` using the provided `seed`.
pub fn from_bytes_seed<'a, T>(seed: T, bytes: &'a [u8]) -> Result<T::Value>
where
    T: DeserializeSeed<'a>,
{
    let mut deserializer = Deserializer::new(bytes, crate::MAX_CONTAINER_DEPTH);
    let t = seed.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Same as `from_bytes_seed` but use `limit` as max container depth instead of MAX_CONTAINER_DEPTH`
/// Note that `limit` has to be lower than MAX_CONTAINER_DEPTH
pub fn from_bytes_seed_with_limit<'a, T>(seed: T, bytes: &'a [u8], limit: usize) -> Result<T::Value>
where
    T: DeserializeSeed<'a>,
{
    if limit > crate::MAX_CONTAINER_DEPTH {
        return Err(Error::NotSupported("limit exceeds the max allowed depth"));
    }
    let mut deserializer = Deserializer::new(bytes, limit);
    let t = seed.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Deserialize a type from an implementation of [`Read`].
pub fn from_reader<T>(mut reader: impl Read) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut deserializer = Deserializer::from_reader(&mut reader, crate::MAX_CONTAINER_DEPTH);
    let t = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Same as `from_reader_seed` but use `limit` as max container depth instead of MAX_CONTAINER_DEPTH`
/// Note that `limit` has to be lower than MAX_CONTAINER_DEPTH
pub fn from_reader_with_limit<T>(mut reader: impl Read, limit: usize) -> Result<T>
where
    T: DeserializeOwned,
{
    if limit > crate::MAX_CONTAINER_DEPTH {
        return Err(Error::NotSupported("limit exceeds the max allowed depth"));
    }
    let mut deserializer = Deserializer::from_reader(&mut reader, limit);
    let t = T::deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Deserialize a type from an implementation of [`Read`] using the provided seed
pub fn from_reader_seed<T, V>(seed: T, mut reader: impl Read) -> Result<V>
where
    for<'a> T: DeserializeSeed<'a, Value = V>,
{
    let mut deserializer = Deserializer::from_reader(&mut reader, crate::MAX_CONTAINER_DEPTH);
    let t = seed.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Same as `from_reader_seed` but use `limit` as max container depth instead of MAX_CONTAINER_DEPTH`
/// Note that `limit` has to be lower than MAX_CONTAINER_DEPTH
pub fn from_reader_seed_with_limit<T, V>(seed: T, mut reader: impl Read, limit: usize) -> Result<V>
where
    for<'a> T: DeserializeSeed<'a, Value = V>,
{
    if limit > crate::MAX_CONTAINER_DEPTH {
        return Err(Error::NotSupported("limit exceeds the max allowed depth"));
    }
    let mut deserializer = Deserializer::from_reader(&mut reader, limit);
    let t = seed.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(t)
}

/// Deserialization implementation for BCS
struct Deserializer<R> {
    input: R,
    max_remaining_depth: usize,
}

impl<'de, R: Read> Deserializer<TeeReader<'de, R>> {
    fn from_reader(input: &'de mut R, max_remaining_depth: usize) -> Self {
        Deserializer {
            input: TeeReader::new(input),
            max_remaining_depth,
        }
    }
}

impl<'de> Deserializer<&'de [u8]> {
    /// Creates a new `Deserializer` which will be deserializing the provided
    /// input.
    fn new(input: &'de [u8], max_remaining_depth: usize) -> Self {
        Deserializer {
            input,
            max_remaining_depth,
        }
    }
}

/// A reader that can optionally capture all bytes from an underlying [`Read`]er
struct TeeReader<'de, R> {
    /// the underlying reader
    reader: &'de mut R,
    /// If non-empty, all bytes read from the underlying reader will be captured in the last entry here.
    captured_keys: Vec<Vec<u8>>,
}

impl<'de, R> TeeReader<'de, R> {
    /// Wraps the provided reader in a new [`TeeReader`].
    pub fn new(reader: &'de mut R) -> Self {
        Self {
            reader,
            captured_keys: Vec::new(),
        }
    }
}

impl<'de, R: Read> Read for TeeReader<'de, R> {
    fn read(&mut self, buf: &mut [u8]) -> crate::io::Result<usize> {
        let bytes_read = self.reader.read(buf)?;
        if let Some(buffer) = self.captured_keys.last_mut() {
            buffer.extend_from_slice(&buf[..bytes_read]);
        }
        Ok(bytes_read)
    }
}

trait BcsDeserializer<'de> {
    type MaybeBorrowedBytes: AsRef<[u8]>;

    fn fill_slice(&mut self, slice: &mut [u8]) -> Result<()>;

    fn parse_and_visit_str<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>;

    fn parse_and_visit_bytes<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>;

    fn next_key_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<(V::Value, Self::MaybeBorrowedBytes)>;

    /// The `Deserializer::end` method should be called after a type has been
    /// fully deserialized. This allows the `Deserializer` to validate that
    /// the there are no more bytes remaining in the input stream.
    fn end(&mut self) -> Result<()>;

    fn parse_bool(&mut self) -> Result<bool> {
        let byte = self.next()?;

        match byte {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::ExpectedBoolean),
        }
    }

    fn next(&mut self) -> Result<u8> {
        let mut byte = [0u8; 1];
        self.fill_slice(&mut byte)?;
        Ok(byte[0])
    }

    fn parse_u8(&mut self) -> Result<u8> {
        self.next()
    }

    fn parse_u16(&mut self) -> Result<u16> {
        let mut le_bytes = [0; 2];
        self.fill_slice(&mut le_bytes)?;
        Ok(u16::from_le_bytes(le_bytes))
    }

    fn parse_u32(&mut self) -> Result<u32> {
        let mut le_bytes = [0; 4];
        self.fill_slice(&mut le_bytes)?;
        Ok(u32::from_le_bytes(le_bytes))
    }

    fn parse_u64(&mut self) -> Result<u64> {
        let mut le_bytes = [0; 8];
        self.fill_slice(&mut le_bytes)?;
        Ok(u64::from_le_bytes(le_bytes))
    }

    fn parse_u128(&mut self) -> Result<u128> {
        let mut le_bytes = [0; 16];
        self.fill_slice(&mut le_bytes)?;
        Ok(u128::from_le_bytes(le_bytes))
    }

    fn parse_u32_from_uleb128(&mut self) -> Result<u32> {
        let mut value: u64 = 0;
        for shift in (0..32).step_by(7) {
            let byte = self.next()?;
            let digit = byte & 0x7f;
            value |= u64::from(digit) << shift;
            // If the highest bit of `byte` is 0, return the final value.
            if digit == byte {
                if shift > 0 && digit == 0 {
                    // We only accept canonical ULEB128 encodings, therefore the
                    // heaviest (and last) base-128 digit must be non-zero.
                    return Err(Error::NonCanonicalUleb128Encoding);
                }
                // Decoded integer must not overflow.
                return u32::try_from(value)
                    .map_err(|_| Error::IntegerOverflowDuringUleb128Decoding);
            }
        }
        // Decoded integer must not overflow.
        Err(Error::IntegerOverflowDuringUleb128Decoding)
    }

    fn parse_length(&mut self) -> Result<usize> {
        let len = self.parse_u32_from_uleb128()? as usize;
        if len > crate::MAX_SEQUENCE_LENGTH {
            return Err(Error::ExceededMaxLen(len));
        }
        Ok(len)
    }
}

impl<'de, R: Read> Deserializer<TeeReader<'de, R>> {
    fn parse_vec(&mut self) -> Result<Vec<u8>> {
        let len = self.parse_length()?;
        let mut output = vec![0; len];
        self.fill_slice(&mut output)?;
        Ok(output)
    }

    fn parse_string(&mut self) -> Result<String> {
        let vec = self.parse_vec()?;
        String::from_utf8(vec).map_err(|_| Error::Utf8)
    }
}

impl<'de, R: Read> BcsDeserializer<'de> for Deserializer<TeeReader<'de, R>> {
    type MaybeBorrowedBytes = Vec<u8>;

    fn fill_slice(&mut self, slice: &mut [u8]) -> Result<()> {
        Ok(self.input.read_exact(slice)?)
    }

    fn parse_and_visit_str<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
    }

    fn parse_and_visit_bytes<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.parse_vec()?)
    }

    fn next_key_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<(V::Value, Self::MaybeBorrowedBytes)> {
        self.input.captured_keys.push(Vec::new());
        let key_value = seed.deserialize(&mut *self)?;
        let key_bytes = self.input.captured_keys.pop().unwrap();
        if let Some(previous_key) = self.input.captured_keys.last_mut() {
            previous_key.extend_from_slice(&key_bytes);
        }
        Ok((key_value, key_bytes))
    }

    fn end(&mut self) -> Result<()> {
        let mut byte = [0u8; 1];
        match self.input.read_exact(&mut byte) {
            Ok(_) => Err(Error::RemainingInput),
            Err(e) if e.kind() == crate::io::ErrorKind::UnexpectedEof => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl<'de> BcsDeserializer<'de> for Deserializer<&'de [u8]> {
    type MaybeBorrowedBytes = &'de [u8];
    fn next(&mut self) -> Result<u8> {
        let byte = self.peek()?;
        self.input = &self.input[1..];
        Ok(byte)
    }

    fn fill_slice(&mut self, slice: &mut [u8]) -> Result<()> {
        for byte in slice {
            *byte = self.next()?;
        }
        Ok(())
    }

    fn parse_and_visit_str<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.parse_string()?)
    }

    fn parse_and_visit_bytes<V>(&mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.parse_bytes()?)
    }

    fn next_key_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<(V::Value, Self::MaybeBorrowedBytes)> {
        let previous_input_slice = self.input;
        let key_value = seed.deserialize(&mut *self)?;
        let key_len = previous_input_slice.len().saturating_sub(self.input.len());
        let key_bytes = &previous_input_slice[..key_len];
        Ok((key_value, key_bytes))
    }

    fn end(&mut self) -> Result<()> {
        if self.input.is_empty() {
            Ok(())
        } else {
            Err(Error::RemainingInput)
        }
    }
}

impl<'de> Deserializer<&'de [u8]> {
    fn peek(&mut self) -> Result<u8> {
        self.input.first().copied().ok_or(Error::Eof)
    }

    fn parse_bytes(&mut self) -> Result<&'de [u8]> {
        let len = self.parse_length()?;
        let slice = self.input.get(..len).ok_or(Error::Eof)?;
        self.input = &self.input[len..];
        Ok(slice)
    }

    fn parse_string(&mut self) -> Result<&'de str> {
        let slice = self.parse_bytes()?;
        str::from_utf8(slice).map_err(|_| Error::Utf8)
    }
}

impl<R> Deserializer<R> {
    fn enter_named_container(&mut self, name: &'static str) -> Result<()> {
        if self.max_remaining_depth == 0 {
            return Err(Error::ExceededContainerDepthLimit(name));
        }
        self.max_remaining_depth -= 1;
        Ok(())
    }

    fn leave_named_container(&mut self) {
        self.max_remaining_depth += 1;
    }
}

impl<'de, 'a, R> de::Deserializer<'de> for &'a mut Deserializer<R>
where
    Deserializer<R>: BcsDeserializer<'de>,
{
    type Error = Error;

    // BCS is not a self-describing format so we can't implement `deserialize_any`
    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported("deserialize_any"))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse_u8()? as i8)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_u16()? as i16)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse_u32()? as i32)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse_u64()? as i64)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i128(self.parse_u128()? as i128)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse_u16()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_u64()?)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u128(self.parse_u128()?)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported("deserialize_f32"))
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported("deserialize_f64"))
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported("deserialize_char"))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.parse_and_visit_str(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.parse_and_visit_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.parse_and_visit_bytes(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.parse_and_visit_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let byte = self.next()?;

        match byte {
            0 => visitor.visit_none(),
            1 => visitor.visit_some(self),
            _ => Err(Error::ExpectedOption),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.enter_named_container(name)?;
        let r = self.deserialize_unit(visitor);
        self.leave_named_container();
        r
    }

    fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.enter_named_container(name)?;
        let r = visitor.visit_newtype_struct(&mut *self);
        self.leave_named_container();
        r
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = self.parse_length()?;
        visitor.visit_seq(SeqDeserializer::new(self, len))
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(SeqDeserializer::new(self, len))
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.enter_named_container(name)?;
        let r = visitor.visit_seq(SeqDeserializer::new(self, len));
        self.leave_named_container();
        r
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = self.parse_length()?;
        visitor.visit_map(MapDeserializer::new(self, len))
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.enter_named_container(name)?;
        let r = visitor.visit_seq(SeqDeserializer::new(self, fields.len()));
        self.leave_named_container();
        r
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.enter_named_container(name)?;
        let r = visitor.visit_enum(&mut *self);
        self.leave_named_container();
        r
    }

    // BCS does not utilize identifiers, so throw them away
    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(_visitor)
    }

    // BCS is not a self-describing format so we can't implement `deserialize_ignored_any`
    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotSupported("deserialize_ignored_any"))
    }

    // BCS is not a human readable format
    fn is_human_readable(&self) -> bool {
        false
    }
}

struct SeqDeserializer<'a, R> {
    de: &'a mut Deserializer<R>,
    remaining: usize,
}
#[allow(clippy::needless_borrow)]
impl<'a, R> SeqDeserializer<'a, R> {
    fn new(de: &'a mut Deserializer<R>, remaining: usize) -> Self {
        Self { de, remaining }
    }
}

impl<'a, 'de, R> de::SeqAccess<'de> for SeqDeserializer<'a, R>
where
    Deserializer<R>: BcsDeserializer<'de>,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            Ok(None)
        } else {
            self.remaining -= 1;
            seed.deserialize(&mut *self.de).map(Some)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

struct MapDeserializer<'a, R, B> {
    de: &'a mut Deserializer<R>,
    remaining: usize,
    previous_key_bytes: Option<B>,
}

impl<'a, R, B> MapDeserializer<'a, R, B> {
    fn new(de: &'a mut Deserializer<R>, remaining: usize) -> Self {
        Self {
            de,
            remaining,
            previous_key_bytes: None,
        }
    }
}

impl<'de, 'a, R, B: AsRef<[u8]>> de::MapAccess<'de> for MapDeserializer<'a, R, B>
where
    Deserializer<R>: BcsDeserializer<'de, MaybeBorrowedBytes = B>,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        match self.remaining.checked_sub(1) {
            None => Ok(None),
            Some(remaining) => {
                let (key_value, key_bytes) = self.de.next_key_seed(seed)?;
                if let Some(previous_key_bytes) = &self.previous_key_bytes {
                    if previous_key_bytes.as_ref() >= key_bytes.as_ref() {
                        return Err(Error::NonCanonicalMap);
                    }
                }
                self.remaining = remaining;
                self.previous_key_bytes = Some(key_bytes);
                Ok(Some(key_value))
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

impl<'de, 'a, R> de::EnumAccess<'de> for &'a mut Deserializer<R>
where
    Deserializer<R>: BcsDeserializer<'de>,
{
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let variant_index = self.parse_u32_from_uleb128()?;
        let result: Result<V::Value> = seed.deserialize(variant_index.into_deserializer());
        Ok((result?, self))
    }
}

impl<'de, 'a, R> de::VariantAccess<'de> for &'a mut Deserializer<R>
where
    Deserializer<R>: BcsDeserializer<'de>,
{
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
    }
}
