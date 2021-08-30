use schemars::JsonSchema;
use serde::{de, ser, Deserialize, Deserializer, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::iter::Sum;
use std::ops::{self, Shr};

use crate::errors::{
    ConversionOverflowError, DivideByZeroError, OverflowError, OverflowOperation, StdError,
};
use crate::Uint128;

/// This module is purely a workaround that lets us ignore lints for all the code
/// the `construct_uint!` macro generates.
#[allow(clippy::all)]
mod uints {
    uint::construct_uint! {
        pub struct U256(4);
    }
}

/// Used internally - we don't want to leak this type since we might change
/// the implementation in the future.
use uints::U256;

/// An implementation of u256 that is using strings for JSON encoding/decoding,
/// such that the full u256 range can be used for clients that convert JSON numbers to floats,
/// like JavaScript and jq.
///
/// # Examples
///
/// Use `from` to create instances out of primitive uint types or `new` to provide big
/// endian bytes:
///
/// ```
/// # use cosmwasm_std::Uint256;
/// let a = Uint256::from(258u128);
/// let b = Uint256::new([
///     0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
///     0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
///     0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
///     0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 1u8, 2u8,
/// ]);
/// assert_eq!(a, b);
/// ```
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
pub struct Uint256(#[schemars(with = "String")] U256);

impl Uint256 {
    pub const MAX: Uint256 = Uint256(U256::MAX);

    /// Creates a Uint256(value) from a big endian representation. It's just an alias for
    /// `from_big_endian`.
    pub fn new(value: [u8; 32]) -> Self {
        Self::from_be_bytes(value)
    }

    /// Creates a Uint256(0)
    pub const fn zero() -> Self {
        Uint256(U256::zero())
    }

    pub fn from_be_bytes(value: [u8; 32]) -> Self {
        Uint256(U256::from_big_endian(&value))
    }

    pub fn from_le_bytes(value: [u8; 32]) -> Self {
        Uint256(U256::from_little_endian(&value))
    }

    /// Returns a copy of the number as big endian bytes.
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut result = [0u8; 32];
        self.0.to_big_endian(&mut result);
        result
    }

    /// Returns a copy of the number as little endian bytes.
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut result = [0u8; 32];
        self.0.to_little_endian(&mut result);
        result
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn checked_add(self, other: Self) -> Result<Self, OverflowError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Add, self, other))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, OverflowError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Sub, self, other))
    }

    pub fn checked_mul(self, other: Self) -> Result<Self, OverflowError> {
        self.0
            .checked_mul(other.0)
            .map(Self)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Mul, self, other))
    }

    pub fn checked_div(self, other: Self) -> Result<Self, DivideByZeroError> {
        self.0
            .checked_div(other.0)
            .map(Self)
            .ok_or_else(|| DivideByZeroError::new(self))
    }

    pub fn checked_rem(self, other: Self) -> Result<Self, DivideByZeroError> {
        self.0
            .checked_rem(other.0)
            .map(Self)
            .ok_or_else(|| DivideByZeroError::new(self))
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn saturating_mul(self, other: Self) -> Self {
        Self(self.0.saturating_mul(other.0))
    }
}

impl From<u128> for Uint256 {
    fn from(val: u128) -> Self {
        Uint256(val.into())
    }
}

impl From<u64> for Uint256 {
    fn from(val: u64) -> Self {
        Uint256(val.into())
    }
}

impl From<u32> for Uint256 {
    fn from(val: u32) -> Self {
        Uint256(val.into())
    }
}

impl From<u16> for Uint256 {
    fn from(val: u16) -> Self {
        Uint256(val.into())
    }
}

impl From<u8> for Uint256 {
    fn from(val: u8) -> Self {
        Uint256(val.into())
    }
}

impl TryFrom<Uint256> for Uint128 {
    type Error = ConversionOverflowError;

    fn try_from(value: Uint256) -> Result<Self, Self::Error> {
        Ok(Uint128::new(value.0.try_into().map_err(|_| {
            ConversionOverflowError::new("Uint256", "Uint128", value.to_string())
        })?))
    }
}

impl TryFrom<&str> for Uint256 {
    type Error = StdError;

    fn try_from(val: &str) -> Result<Self, Self::Error> {
        match U256::from_dec_str(val) {
            Ok(u) => Ok(Uint256(u)),
            Err(e) => Err(StdError::generic_err(format!("Parsing u256: {}", e))),
        }
    }
}

impl From<Uint256> for String {
    fn from(original: Uint256) -> Self {
        original.to_string()
    }
}

impl fmt::Display for Uint256 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ops::Add<Uint256> for Uint256 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Uint256(self.0.checked_add(rhs.0).unwrap())
    }
}

impl<'a> ops::Add<&'a Uint256> for Uint256 {
    type Output = Self;

    fn add(self, rhs: &'a Uint256) -> Self {
        Uint256(self.0.checked_add(rhs.0).unwrap())
    }
}

impl ops::Sub<Uint256> for Uint256 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Uint256(self.0.checked_sub(rhs.0).unwrap())
    }
}

impl<'a> ops::Sub<&'a Uint256> for Uint256 {
    type Output = Self;

    fn sub(self, rhs: &'a Uint256) -> Self {
        Uint256(self.0.checked_sub(rhs.0).unwrap())
    }
}

impl ops::Div<Uint256> for Uint256 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0.checked_div(rhs.0).unwrap())
    }
}

impl<'a> ops::Div<&'a Uint256> for Uint256 {
    type Output = Self;

    fn div(self, rhs: &'a Uint256) -> Self::Output {
        Self(self.0.checked_div(rhs.0).unwrap())
    }
}

impl ops::Mul<Uint256> for Uint256 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0.checked_mul(rhs.0).unwrap())
    }
}

impl<'a> ops::Mul<&'a Uint256> for Uint256 {
    type Output = Self;

    fn mul(self, rhs: &'a Uint256) -> Self::Output {
        Self(self.0.checked_mul(rhs.0).unwrap())
    }
}

impl ops::Shr<u32> for Uint256 {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self::Output {
        if rhs > 256 {
            panic!(
                "right shift error: {} is larger than the number of bits in Uint256",
                rhs
            );
        }

        Self(self.0.shr(rhs))
    }
}

impl<'a> ops::Shr<&'a u32> for Uint256 {
    type Output = Self;

    fn shr(self, rhs: &'a u32) -> Self::Output {
        if *rhs > 256 {
            panic!(
                "right shift error: {} is larger than the number of bits in Uint256",
                rhs
            );
        }

        Self(self.0.shr(*rhs))
    }
}

impl ops::AddAssign<Uint256> for Uint256 {
    fn add_assign(&mut self, rhs: Uint256) {
        self.0 = self.0.checked_add(rhs.0).unwrap();
    }
}

impl<'a> ops::AddAssign<&'a Uint256> for Uint256 {
    fn add_assign(&mut self, rhs: &'a Uint256) {
        self.0 = self.0.checked_add(rhs.0).unwrap();
    }
}

impl ops::SubAssign<Uint256> for Uint256 {
    fn sub_assign(&mut self, rhs: Uint256) {
        self.0 = self.0.checked_sub(rhs.0).unwrap();
    }
}

impl<'a> ops::SubAssign<&'a Uint256> for Uint256 {
    fn sub_assign(&mut self, rhs: &'a Uint256) {
        self.0 = self.0.checked_sub(rhs.0).unwrap();
    }
}

impl ops::DivAssign<Uint256> for Uint256 {
    fn div_assign(&mut self, rhs: Self) {
        self.0 = self.0.checked_div(rhs.0).unwrap();
    }
}

impl<'a> ops::DivAssign<&'a Uint256> for Uint256 {
    fn div_assign(&mut self, rhs: &'a Uint256) {
        self.0 = self.0.checked_div(rhs.0).unwrap();
    }
}

impl ops::MulAssign<Uint256> for Uint256 {
    fn mul_assign(&mut self, rhs: Self) {
        self.0 = self.0.checked_mul(rhs.0).unwrap();
    }
}

impl<'a> ops::MulAssign<&'a Uint256> for Uint256 {
    fn mul_assign(&mut self, rhs: &'a Uint256) {
        self.0 = self.0.checked_mul(rhs.0).unwrap();
    }
}

impl ops::ShrAssign<u32> for Uint256 {
    fn shr_assign(&mut self, rhs: u32) {
        if rhs > 256 {
            panic!(
                "right shift error: {} is larger than the number of bits in Uint256",
                rhs
            );
        }

        self.0 = self.0.shr(rhs);
    }
}

impl<'a> ops::ShrAssign<&'a u32> for Uint256 {
    fn shr_assign(&mut self, rhs: &'a u32) {
        if *rhs > 256 {
            panic!(
                "right shift error: {} is larger than the number of bits in Uint256",
                rhs
            );
        }

        self.0 = self.0.shr(*rhs);
    }
}

impl Serialize for Uint256 {
    /// Serializes as an integer string using base 10
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Uint256 {
    /// Deserialized from an integer string using base 10
    fn deserialize<D>(deserializer: D) -> Result<Uint256, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Uint256Visitor)
    }
}

struct Uint256Visitor;

impl<'de> de::Visitor<'de> for Uint256Visitor {
    type Value = Uint256;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string-encoded integer")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Uint256::try_from(v).map_err(|e| E::custom(format!("invalid Uint256 '{}' - {}", v, e)))
    }
}

impl Sum<Uint256> for Uint256 {
    fn sum<I: Iterator<Item = Uint256>>(iter: I) -> Self {
        iter.fold(Uint256::zero(), ops::Add::add)
    }
}

impl<'a> Sum<&'a Uint256> for Uint256 {
    fn sum<I: Iterator<Item = &'a Uint256>>(iter: I) -> Self {
        iter.fold(Uint256::zero(), ops::Add::add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_slice, to_vec};

    #[test]
    fn uint256_construct() {
        let original = Uint256::new([1; 32]);
        let a: [u8; 32] = original.to_be_bytes();
        assert_eq!(a, [1; 32]);
    }

    #[test]
    fn uint256_convert_from() {
        let a = Uint256::from(5u128);
        assert_eq!(a.0, U256::from(5));

        let a = Uint256::from(5u64);
        assert_eq!(a.0, U256::from(5));

        let a = Uint256::from(5u32);
        assert_eq!(a.0, U256::from(5));

        let a = Uint256::from(5u16);
        assert_eq!(a.0, U256::from(5));

        let a = Uint256::from(5u8);
        assert_eq!(a.0, U256::from(5));

        let result = Uint256::try_from("34567");
        assert_eq!(result.unwrap().0, U256::from_dec_str("34567").unwrap());

        let result = Uint256::try_from("1.23");
        assert!(result.is_err());
    }

    #[test]
    fn uint256_convert_to_uint128() {
        let source = Uint256::from(42u128);
        let target = Uint128::try_from(source);
        assert_eq!(target, Ok(Uint128::new(42u128)));

        let source = Uint256::MAX;
        let target = Uint128::try_from(source);
        assert_eq!(
            target,
            Err(ConversionOverflowError::new(
                "Uint256",
                "Uint128",
                Uint256::MAX.to_string()
            ))
        );
    }

    #[test]
    fn uint256_implements_display() {
        let a = Uint256::from(12345u32);
        assert_eq!(format!("Embedded: {}", a), "Embedded: 12345");
        assert_eq!(a.to_string(), "12345");

        let a = Uint256::zero();
        assert_eq!(format!("Embedded: {}", a), "Embedded: 0");
        assert_eq!(a.to_string(), "0");
    }

    #[test]
    fn uint256_is_zero_works() {
        assert!(Uint256::zero().is_zero());
        assert!(Uint256(U256::from(0)).is_zero());

        assert!(!Uint256::from(1u32).is_zero());
        assert!(!Uint256::from(123u32).is_zero());
    }

    #[test]
    fn uint256_json() {
        let orig = Uint256::from(1234567890987654321u128);
        let serialized = to_vec(&orig).unwrap();
        assert_eq!(serialized.as_slice(), b"\"1234567890987654321\"");
        let parsed: Uint256 = from_slice(&serialized).unwrap();
        assert_eq!(parsed, orig);
    }

    #[test]
    fn uint256_compare() {
        let a = Uint256::from(12345u32);
        let b = Uint256::from(23456u32);

        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, Uint256::from(12345u32));
    }

    #[test]
    #[allow(clippy::op_ref)]
    fn uint256_math() {
        let a = Uint256::from(12345u32);
        let b = Uint256::from(23456u32);

        // test + with owned and reference right hand side
        assert_eq!(a + b, Uint256::from(35801u32));
        assert_eq!(a + &b, Uint256::from(35801u32));

        // test - with owned and reference right hand side
        assert_eq!(b - a, Uint256::from(11111u32));
        assert_eq!(b - &a, Uint256::from(11111u32));

        // test += with owned and reference right hand side
        let mut c = Uint256::from(300000u32);
        c += b;
        assert_eq!(c, Uint256::from(323456u32));
        let mut d = Uint256::from(300000u32);
        d += &b;
        assert_eq!(d, Uint256::from(323456u32));

        // test -= with owned and reference right hand side
        let mut c = Uint256::from(300000u32);
        c -= b;
        assert_eq!(c, Uint256::from(276544u32));
        let mut d = Uint256::from(300000u32);
        d -= &b;
        assert_eq!(d, Uint256::from(276544u32));

        // error result on underflow (- would produce negative result)
        let underflow_result = a.checked_sub(b);
        let OverflowError {
            operand1, operand2, ..
        } = underflow_result.unwrap_err();
        assert_eq!((operand1, operand2), (a.to_string(), b.to_string()));
    }

    #[test]
    #[should_panic]
    fn uint256_add_overflow_panics() {
        let max = Uint256::new([255u8; 32]);
        let _ = max + Uint256::from(12u32);
    }

    #[test]
    #[should_panic]
    fn uint256_sub_overflow_panics() {
        let _ = Uint256::from(1u32) - Uint256::from(2u32);
    }

    #[test]
    fn sum_works() {
        let nums = vec![
            Uint256::from(17u32),
            Uint256::from(123u32),
            Uint256::from(540u32),
            Uint256::from(82u32),
        ];
        let expected = Uint256::from(762u32);

        let sum_as_ref = nums.iter().sum();
        assert_eq!(expected, sum_as_ref);

        let sum_as_owned = nums.into_iter().sum();
        assert_eq!(expected, sum_as_owned);
    }

    #[test]
    fn uint256_methods() {
        // checked_*
        assert!(matches!(
            Uint256::MAX.checked_add(Uint256::from(1u32)),
            Err(OverflowError { .. })
        ));
        assert!(matches!(
            Uint256::from(0u32).checked_sub(Uint256::from(1u32)),
            Err(OverflowError { .. })
        ));
        assert!(matches!(
            Uint256::MAX.checked_mul(Uint256::from(2u32)),
            Err(OverflowError { .. })
        ));
        assert!(matches!(
            Uint256::MAX.checked_div(Uint256::from(0u32)),
            Err(DivideByZeroError { .. })
        ));
        assert!(matches!(
            Uint256::MAX.checked_rem(Uint256::from(0u32)),
            Err(DivideByZeroError { .. })
        ));

        // saturating_*
        assert_eq!(
            Uint256::MAX.saturating_add(Uint256::from(1u32)),
            Uint256::MAX
        );
        assert_eq!(
            Uint256::from(0u32).saturating_sub(Uint256::from(1u32)),
            Uint256::from(0u32)
        );
        assert_eq!(
            Uint256::MAX.saturating_mul(Uint256::from(2u32)),
            Uint256::MAX
        );
    }
}
