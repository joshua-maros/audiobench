use std::fmt::{self, Debug, Formatter};

/// Represents a data type which may fall within the specified bounds depending on how the program
/// is compiled.
#[derive(Clone, PartialEq)]
pub struct DataType {
    pub actual_type: Option<SpecificDataType>,
    pub bounds: Bounds,
}

impl DataType {
    pub fn unbounded() -> Self {
        Self {
            actual_type: None,
            bounds: Bounds::Unbounded,
        }
    }

    pub fn specific(only_valid_type: SpecificDataType) -> Self {
        Self {
            actual_type: Some(only_valid_type.clone()),
            bounds: Bounds::LowerAndUpper(only_valid_type.clone(), only_valid_type),
        }
    }

    /// Returns this data type with the supplied function applied to its type bounds and actual
    /// type, if they exist.
    pub fn map_ref(&self, fun: impl Fn(&SpecificDataType) -> SpecificDataType) -> Self {
        let actual_type = self.actual_type.as_ref().map(|x| fun(x));
        let (lower, upper) = self.bounds.as_tuple();
        let bounds = Bounds::from_tuple((lower.map(|x| fun(x)), upper.map(|x| fun(x))));
        Self {
            actual_type,
            bounds,
        }
    }

    /// Returns this data type with the supplied function applied to its type bounds and actual
    /// type, if they exist.
    pub fn map(self, fun: impl Fn(SpecificDataType) -> SpecificDataType) -> Self {
        let actual_type = self.actual_type.map(|x| fun(x));
        let (lower, upper) = self.bounds.into_tuple();
        let bounds = Bounds::from_tuple((lower.map(|x| fun(x)), upper.map(|x| fun(x))));
        Self {
            actual_type,
            bounds,
        }
    }

    pub fn with_different_base(&self, new_base: SpecificDataType) -> Self {
        self.map_ref(|typ| typ.with_different_base(new_base.clone()))
    }
}

impl From<SpecificDataType> for DataType {
    fn from(specific: SpecificDataType) -> Self {
        Self::specific(specific)
    }
}

impl Debug for DataType {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "{:?} ({:?})", self.bounds, self.actual_type)
    }
}

#[derive(Clone, PartialEq)]
pub enum Bounds {
    Unbounded,
    Upper(SpecificDataType),
    LowerAndUpper(SpecificDataType, SpecificDataType),
}

impl Bounds {
    pub fn from_tuple(bounds: (Option<SpecificDataType>, Option<SpecificDataType>)) -> Self {
        match bounds {
            (None, None) => Self::Unbounded,
            (None, Some(upper)) => Self::Upper(upper),
            (Some(lower), Some(upper)) => Self::LowerAndUpper(lower, upper),
            (Some(..), None) => {
                panic!("Cannot have a type bound with a lower bound but no upper bound.")
            }
        }
    }

    pub fn as_tuple(&self) -> (Option<&SpecificDataType>, Option<&SpecificDataType>) {
        match self {
            Self::Unbounded => (None, None),
            Self::Upper(upper) => (None, Some(upper)),
            Self::LowerAndUpper(lower, upper) => (Some(lower), Some(upper)),
        }
    }

    pub fn into_tuple(self) -> (Option<SpecificDataType>, Option<SpecificDataType>) {
        match self {
            Self::Unbounded => (None, None),
            Self::Upper(upper) => (None, Some(upper)),
            Self::LowerAndUpper(lower, upper) => (Some(lower), Some(upper)),
        }
    }
}

impl Debug for Bounds {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Self::Unbounded => write!(formatter, "<>"),
            Self::Upper(upper) => write!(formatter, "<{:?}>", upper),
            Self::LowerAndUpper(lower, upper) => write!(formatter, "<{:?}, {:?}>", lower, upper),
        }
    }
}

/// Represents an unambiguous data type. This is used to represent the lower and upper bounds of
/// a data type as well as the actual type if it is known.
#[derive(Clone, PartialEq)]
pub enum SpecificDataType {
    Bool,
    Int,
    Float,
    Void,
    DataType,
    Macro,
    Array(usize, Box<SpecificDataType>),
}

impl SpecificDataType {
    pub fn equivalent(&self, other: &Self) -> bool {
        match self {
            // If it's a basic type, just check if it is equal to the other one.
            Self::Bool | Self::Int | Self::Float | Self::Void | Self::DataType | Self::Macro => {
                self == other
            }
            Self::Array(my_size, my_etype) => {
                if let Self::Array(size, etype) = other {
                    my_size == size && my_etype.equivalent(etype)
                } else {
                    false
                }
            }
        }
    }

    pub fn make_array(dims: &[usize], base: Self) -> Self {
        if dims.len() > 0 {
            Self::Array(dims[0], Box::new(Self::make_array(&dims[1..], base)))
        } else {
            base
        }
    }

    fn collect_dims_impl(&self, dims: &mut Vec<usize>) {
        if let Self::Array(size, btype) = self {
            dims.push(*size);
            btype.collect_dims_impl(dims);
        }
    }

    pub fn collect_dims(&self) -> Vec<usize> {
        let mut dims = Vec::new();
        self.collect_dims_impl(&mut dims);
        dims
    }

    pub fn with_different_base(&self, new_base: SpecificDataType) -> Self {
        match self {
            Self::Array(size, etyp) => {
                Self::Array(*size, Box::new(etyp.with_different_base(new_base)))
            }
            _ => new_base,
        }
    }
}

impl Debug for SpecificDataType {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Self::Bool => write!(formatter, "BOOL"),
            Self::Int => write!(formatter, "INT"),
            Self::Float => write!(formatter, "FLOAT"),
            Self::Void => write!(formatter, "VOID"),
            Self::DataType => write!(formatter, "DATA_TYPE"),
            Self::Macro => write!(formatter, "MACRO"),
            Self::Array(size, etype) => write!(formatter, "[{}]{:?}", size, etype),
        }
    }
}
