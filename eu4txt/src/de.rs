use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, forward_to_deserialize_any};

use crate::{EU4TxtAstItem, EU4TxtParseNode};
use std::fmt;

pub struct Deserializer<'de> {
    input: &'de EU4TxtParseNode,
    // We might need state to track if we are iterating children
    child_iter: std::slice::Iter<'de, EU4TxtParseNode>,
}

impl<'de> Deserializer<'de> {
    pub fn from_node(input: &'de EU4TxtParseNode) -> Self {
        Deserializer {
            input,
            child_iter: input.children.iter(),
        }
    }
}

pub fn from_node<'a, T>(node: &'a EU4TxtParseNode) -> Result<T, String>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_node(node);
    let t = T::deserialize(&mut deserializer).map_err(|e| e.to_string())?;
    Ok(t)
}

// Error handling omitted for brevity, using String for now
#[derive(Debug)]
pub struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Error {}
impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.input.entry {
            EU4TxtAstItem::Identifier(s) | EU4TxtAstItem::StringValue(s) => visitor.visit_str(s),
            EU4TxtAstItem::IntValue(i) => visitor.visit_i32(*i),
            EU4TxtAstItem::FloatValue(f) => visitor.visit_f32(*f),
            EU4TxtAstItem::AssignmentList => {
                // It's a container. Could be a Seq or a Map (Struct).
                // We don't know without a hint. But usually for any, we can try map?
                // Or we scan children. AST doesn't differentiate object vs array well unless we check if children are assignments.
                // Heuristic: If first child is Assignment, it's a Map. Else Seq.
                if self
                    .input
                    .children
                    .first()
                    .is_some_and(|first| matches!(first.entry, EU4TxtAstItem::Assignment))
                {
                    return self.deserialize_map(visitor);
                }
                self.deserialize_seq(visitor)
            }
            EU4TxtAstItem::Assignment => {
                // Assignment is strictly Key = Value.
                // Usually handled by MapAccess, but if we are here, maybe we want the Val?
                // Or maybe a tuple?
                Err(Error(
                    "Unexpected Assignment in deserialize_any".to_string(),
                ))
            }
            _ => Err(Error(format!(
                "Unimplemented deserialize_any for {:?}",
                self.input.entry
            ))),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.input.entry {
            EU4TxtAstItem::Identifier(s) => {
                if s == "yes" {
                    visitor.visit_bool(true)
                } else if s == "no" {
                    visitor.visit_bool(false)
                } else {
                    Err(Error(format!("Invalid bool: {}", s)))
                }
            }
            _ => Err(Error("Not a bool".to_string())),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.input.entry {
            EU4TxtAstItem::IntValue(i) => visitor.visit_i32(*i),
            _ => Err(Error("Not an i32".to_string())),
        }
    }
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.input.entry {
            EU4TxtAstItem::FloatValue(f) => visitor.visit_f32(*f),
            EU4TxtAstItem::IntValue(i) => visitor.visit_f32(*i as f32), // gentle coercion
            _ => Err(Error("Not an f32".to_string())),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.input.entry {
            EU4TxtAstItem::Identifier(s) | EU4TxtAstItem::StringValue(s) => visitor.visit_str(s),
            _ => Err(Error("Not a string".to_string())),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(CommaSeparated::new(&mut self.child_iter))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(CommaSeparated::new(&mut self.child_iter))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // If we exist, we are Some. If we are an empty brace?
        // Actually, logic for Option usually happens in MapAccess (check if key exists).
        // If we are here, we have a value.
        visitor.visit_some(self)
    }

    forward_to_deserialize_any! {
        i8 i16 i64 u8 u16 u32 u64 f64 char bytes byte_buf unit unit_struct newtype_struct tuple
        tuple_struct enum identifier ignored_any
    }
}

// Iterator for Seq and Map Access
struct CommaSeparated<'a, 'de: 'a> {
    iter: &'a mut std::slice::Iter<'de, EU4TxtParseNode>,
    value: Option<&'de EU4TxtParseNode>,
}

impl<'a, 'de> CommaSeparated<'a, 'de> {
    fn new(iter: &'a mut std::slice::Iter<'de, EU4TxtParseNode>) -> Self {
        CommaSeparated { iter, value: None }
    }
}

impl<'de> SeqAccess<'de> for CommaSeparated<'_, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(node) => {
                // If the node is an Assignment (key=val) inside a Seq, what to do?
                // Often sequences are just values: { 1 2 3 }.
                // If it is Key=Val, it might be a list of objects?
                // Just use the node as the deserializer input.
                let mut de = Deserializer::from_node(node);
                seed.deserialize(&mut de).map(Some)
            }
            None => Ok(None),
        }
    }
}

impl<'de> MapAccess<'de> for CommaSeparated<'_, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        // In a Map, we expect children to be Assignments: Key = Val.
        // We peek at the next item.
        // We can't easily peek standard iter, but we can clone it? No.
        // We just get the next item.
        if let Some(node) = self.iter.next() {
            match &node.entry {
                EU4TxtAstItem::Assignment => {
                    // LHS is key. RHS is value.
                    // We store RHS in self.value for next_value call.
                    let key_node = node
                        .children
                        .first()
                        .ok_or(Error("Missing Key".to_string()))?;
                    let val_node = node
                        .children
                        .get(1)
                        .ok_or(Error("Missing Val".to_string()))?;
                    self.value = Some(val_node);

                    let mut de = Deserializer::from_node(key_node);
                    seed.deserialize(&mut de).map(Some)
                }
                _ => {
                    // It's not an assignment. It might be a loose value in a map?
                    // EU4 sometimes has "mixed" bags.
                    // For now, fail or skip?
                    // Fail.
                    Err(Error(format!(
                        "Expected Assignment in Map, got {:?}",
                        node.entry
                    )))
                }
            }
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let val_node = self.value.take().ok_or(Error(
            "MapAccess::next_value called before next_key".to_string(),
        ))?;
        let mut de = Deserializer::from_node(val_node);
        seed.deserialize(&mut de)
    }
}
