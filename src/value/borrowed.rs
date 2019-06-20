///A dom object that references the raw input data to avoid allocations
/// it trades having lifetimes for a gain in performance.
mod cmp;
mod from;
mod serialize;

use crate::number::Number;
use crate::portability::trailingzeroes;
use crate::value::{ValueTrait, ValueType};
use crate::{static_cast_u32, stry, unlikely, Deserializer, ErrorType, Result};
use halfbrown::HashMap;
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::mem;
use std::ops::Index;

const SMALL_STR_LEN: usize = 54;

pub type Map<'v> = HashMap<Cow<'v, str>, Value<'v>>;

/// Parses a slice of butes into a Value dom. This function will
/// rewrite the slice to de-escape strings.
/// As we reference parts of the input slice the resulting dom
/// has the dame lifetime as the slice it was created from.
pub fn to_value<'v>(s: &'v mut [u8]) -> Result<Value<'v>> {
    let mut deserializer = stry!(Deserializer::from_slice(s));
    deserializer.parse_value_borrowed_root()
}

#[derive(Clone)]
pub struct SmallString {
    data: [u8; SMALL_STR_LEN],
    len: u8,
}

impl SmallString {
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { self.data.get_unchecked(..self.len()) }
    }
}

impl fmt::Debug for SmallString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: &str = self.borrow();
        write!(f, "{:?}", s)
    }
}

impl PartialEq for SmallString {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            self.len == other.len
                && self.data.get_unchecked(..self.len()) == other.data.get_unchecked(..other.len())
        }
    }
}

impl PartialEq<str> for SmallString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        unsafe { self.data.get_unchecked(..self.len()) == other.as_bytes() }
    }
}

impl PartialEq<String> for SmallString {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        unsafe { self.data.get_unchecked(..self.len()) == other.as_str().as_bytes() }
    }
}

impl Borrow<str> for SmallString {
    #[inline]
    fn borrow(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.data.get_unchecked(..self.len())) }
    }
}

impl Borrow<[u8]> for SmallString {
    #[inline]
    fn borrow(&self) -> &[u8] {
        unsafe { &self.data.get_unchecked(..self.len()) }
    }
}

impl ToString for SmallString {
    fn to_string(&self) -> String {
        String::from(self.borrow())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value<'v> {
    Null,
    Bool(bool),
    Number(Number),
    String(Cow<'v, str>),
    Array(Vec<Value<'v>>),
    Object(Map<'v>),
    SmallString(SmallString),
}

impl<'v> ValueTrait for Value<'v> {
    type Map = Map<'v>;
    type Array = Vec<Value<'v>>;

    fn get(&self, k: &str) -> Option<&Value<'v>> {
        match self {
            Value::Object(m) => m.get(k),
            _ => None,
        }
    }

    fn get_mut(&mut self, k: &str) -> Option<&mut Value<'v>> {
        match self {
            Value::Object(m) => m.get_mut(k),
            _ => None,
        }
    }

    fn kind(&self) -> ValueType {
        match self {
            Value::Null => ValueType::Null,
            Value::Bool(_) => ValueType::Bool,
            Value::Number(_) => ValueType::I64,
            Value::String(_) => ValueType::String,
            Value::SmallString { .. } => ValueType::String,
            Value::Array(_) => ValueType::Array,
            Value::Object(_) => ValueType::Object,
        }
    }

    fn is_null(&self) -> bool {
        match self {
            Value::Null => true,
            _ => false,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n) => n.as_u64(),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    fn cast_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    fn as_string(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.to_string()),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&Vec<Value<'v>>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    fn as_array_mut(&mut self) -> Option<&mut Vec<Value<'v>>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    fn as_object(&self) -> Option<&Self::Map> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    fn as_object_mut(&mut self) -> Option<&mut Self::Map> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }
}

impl<'v> fmt::Display for Value<'v> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::SmallString(s) => write!(f, "{}", s.to_string()),
            Value::Array(a) => write!(f, "{:?}", a),
            Value::Object(o) => write!(f, "{:?}", o),
        }
    }
}

impl<'g, 'v: 'g> Index<&'g str> for Value<'v> {
    type Output = Value<'v>;
    fn index<'s>(&'s self, index: &'g str) -> &'s Value<'v> {
        static NULL: Value = Value::Null;
        self.get(index).unwrap_or(&NULL)
    }
}

impl<'v> Default for Value<'v> {
    fn default() -> Self {
        Value::Null
    }
}

impl<'de> Deserializer<'de> {
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    pub fn parse_value_borrowed_root(&mut self) -> Result<Value<'de>> {
        match self.next_() {
            b'"' => {
                if self.count_elements() <= SMALL_STR_LEN + 2 {
                    // two for the quotes we don't need to store
                    return self.parse_small_str_().map(Value::from);
                }
                self.parse_str_().map(Value::from)
            }
            b'-' => self.parse_number_root(true).map(Value::Number),
            b'0'..=b'9' => self.parse_number_root(false).map(Value::Number),
            b'n' => Ok(Value::Null),
            b't' => Ok(Value::Bool(true)),
            b'f' => Ok(Value::Bool(false)),
            b'[' => self.parse_array_borrowed(),
            b'{' => self.parse_map_borrowed(),
            _c => Err(self.error(ErrorType::UnexpectedCharacter)),
        }
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_value_borrowed(&mut self) -> Result<Value<'de>> {
        match self.next_() {
            b'"' => {
                // We can only have entered this by being in an object so we know there is
                // something following as we made sure during checking for sizes.;
                if self.count_elements() <= SMALL_STR_LEN + 2 {
                    // two for the quotes we don't need to store
                    return self.parse_small_str_().map(Value::from);
                }
                self.parse_str_().map(Value::from)
            }
            b'-' => self.parse_number_(true).map(Value::Number),
            b'0'..=b'9' => self.parse_number_(false).map(Value::Number),
            b'n' => Ok(Value::Null),
            b't' => Ok(Value::Bool(true)),
            b'f' => Ok(Value::Bool(false)),
            b'[' => self.parse_array_borrowed(),
            b'{' => self.parse_map_borrowed(),
            _c => Err(self.error(ErrorType::UnexpectedCharacter)),
        }
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_array_borrowed(&mut self) -> Result<Value<'de>> {
        let es = self.count_elements();
        if unlikely!(es == 0) {
            self.skip();
            return Ok(Value::Array(Vec::new()));
        }
        let mut res = Vec::with_capacity(es);

        for _i in 0..es {
            res.push(stry!(self.parse_value_borrowed()));
            self.skip();
        }
        Ok(Value::Array(res))
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_map_borrowed(&mut self) -> Result<Value<'de>> {
        // We short cut for empty arrays
        let es = self.count_elements();

        if unlikely!(es == 0) {
            self.skip();
            return Ok(Value::Object(Map::new()));
        }

        let mut res = Map::with_capacity(es);

        // Since we checked if it's empty we know that we at least have one
        // element so we eat this

        for _ in 0..es {
            self.skip();
            let key = stry!(self.parse_short_str_());
            // We have to call parse short str twice since parse_short_str
            // does not move the cursor forward
            self.skip();
            res.insert_nocheck(key.into(), stry!(self.parse_value_borrowed()));
            self.skip();
        }
        Ok(Value::Object(res))
    }
    // We parse a string that's likely to be less then 54 characters and without any
    // fancy in it like object keys
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_small_str_(&mut self) -> Result<Value<'de>> {
        let mut res = SmallString {
            len: 0,
            data: unsafe { mem::uninitialized() },
        };
        let idx = self.iidx + 1;
        let src: &[u8] = unsafe { &self.input.get_unchecked(idx..) };

        //short strings are very common for IDs
        unsafe {
            res.data
                .get_unchecked_mut(..32)
                .clone_from_slice(src.get_unchecked(..32));
        };
        #[allow(clippy::cast_ptr_alignment)]
        let v: __m256i = unsafe { _mm256_loadu_si256(src.as_ptr() as *const __m256i) };
        let bs_bits: u32 = unsafe {
            static_cast_u32!(_mm256_movemask_epi8(_mm256_cmpeq_epi8(
                v,
                _mm256_set1_epi8(b'\\' as i8)
            )))
        };
        let quote_mask = unsafe { _mm256_cmpeq_epi8(v, _mm256_set1_epi8(b'"' as i8)) };
        let quote_bits = unsafe { static_cast_u32!(_mm256_movemask_epi8(quote_mask)) };
        if (bs_bits.wrapping_sub(1) & quote_bits) != 0 {
            let quote_dist: u8 = trailingzeroes(u64::from(quote_bits)) as u8;
            res.len = quote_dist;
            return Ok(Value::SmallString(res));
        } else if (quote_bits.wrapping_sub(1) & bs_bits) == 0 {
            // Nothing bad so far we can do another 22 characters
            unsafe {
                res.data
                    .get_unchecked_mut(32..=SMALL_STR_LEN)
                    .clone_from_slice(src.get_unchecked(32..=SMALL_STR_LEN));
            };
            #[allow(clippy::cast_ptr_alignment)]
            let v: __m256i =
                unsafe { _mm256_loadu_si256(src.get_unchecked(32..).as_ptr() as *const __m256i) };
            let bs_bits: u32 = unsafe {
                static_cast_u32!(_mm256_movemask_epi8(_mm256_cmpeq_epi8(
                    v,
                    _mm256_set1_epi8(b'\\' as i8)
                )))
            };
            let quote_mask = unsafe { _mm256_cmpeq_epi8(v, _mm256_set1_epi8(b'"' as i8)) };
            let quote_bits = unsafe { static_cast_u32!(_mm256_movemask_epi8(quote_mask)) };
            if (bs_bits.wrapping_sub(1) & quote_bits) != 0 {
                let quote_dist: u8 = trailingzeroes(u64::from(quote_bits)) as u8;
                if quote_dist <= 22 {
                    res.len = quote_dist + 32;
                    return Ok(Value::SmallString(res));
                }
            }
        }
        self.parse_str_().map(Value::from)
    }
}
