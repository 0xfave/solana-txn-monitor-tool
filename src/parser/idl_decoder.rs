use crate::types::{IDLInstruction, IDLSchema, IDLTypeReference};
use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tracing::debug;

/// Decoded instruction with parameters
#[derive(Debug, Clone)]
pub struct DecodedInstruction {
    /// Instruction name
    pub name: String,
    /// Instruction discriminator (first 8 bytes)
    pub discriminator: [u8; 8],
    /// Decoded parameters as JSON values
    pub params: HashMap<String, JsonValue>,
}

/// IDL-based instruction decoder
pub struct IdlDecoder {
    /// Maps discriminator to instruction definition
    instruction_map: HashMap<[u8; 8], IDLInstruction>,
}

impl IdlDecoder {
    /// Create a new decoder from an IDL schema
    pub fn new(idl: &IDLSchema) -> Result<Self> {
        let mut instruction_map = HashMap::new();

        for instruction in &idl.instructions {
            if instruction.discriminator.len() == 8 {
                let mut disc = [0u8; 8];
                disc.copy_from_slice(&instruction.discriminator);
                instruction_map.insert(disc, instruction.clone());
            } else {
                debug!(
                    "Skipping instruction {} with invalid discriminator length: {}",
                    instruction.name,
                    instruction.discriminator.len()
                );
            }
        }

        Ok(Self { instruction_map })
    }

    /// Decode instruction data using the IDL
    pub fn decode_instruction(&self, data: &[u8]) -> Result<DecodedInstruction> {
        // Need at least 8 bytes for discriminator
        if data.len() < 8 {
            anyhow::bail!("Instruction data too short: {} bytes", data.len());
        }

        // Extract discriminator
        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&data[0..8]);

        // Find instruction by discriminator
        let instruction = self
            .instruction_map
            .get(&discriminator)
            .context(format!("Unknown instruction discriminator: {:?}", discriminator))?;

        debug!("Decoding instruction: {}", instruction.name);

        // Decode parameters
        let params = self.decode_params(&data[8..], instruction)?;

        Ok(DecodedInstruction { name: instruction.name.clone(), discriminator, params })
    }

    /// Decode instruction parameters
    fn decode_params(&self, data: &[u8], instruction: &IDLInstruction) -> Result<HashMap<String, JsonValue>> {
        let mut params = HashMap::new();
        let mut offset = 0;

        for arg in &instruction.args {
            let (value, bytes_read) = self.decode_type(&data[offset..], &arg.type_ref)?;
            params.insert(arg.name.clone(), value);
            offset += bytes_read;
        }

        Ok(params)
    }

    /// Decode a single type from bytes
    fn decode_type(&self, data: &[u8], type_ref: &IDLTypeReference) -> Result<(JsonValue, usize)> {
        match type_ref {
            IDLTypeReference::Primitive(type_name) => self.decode_primitive(data, type_name),
            IDLTypeReference::Option { option } => self.decode_option(data, option),
            IDLTypeReference::Vec { vec } => self.decode_vec(data, vec),
            IDLTypeReference::Array { array } => self.decode_array(data, array),
            IDLTypeReference::Defined { defined } => {
                // For now, treat defined types as opaque bytes
                // TODO: Implement full struct/enum decoding
                Ok((JsonValue::String(format!("<defined: {:?}>", defined)), 0))
            }
        }
    }

    /// Decode primitive types
    fn decode_primitive(&self, data: &[u8], type_name: &str) -> Result<(JsonValue, usize)> {
        match type_name {
            "u8" => {
                if data.len() < 1 {
                    anyhow::bail!("Not enough data for u8");
                }
                Ok((JsonValue::Number(data[0].into()), 1))
            }
            "u16" => {
                if data.len() < 2 {
                    anyhow::bail!("Not enough data for u16");
                }
                let value = u16::from_le_bytes([data[0], data[1]]);
                Ok((JsonValue::Number(value.into()), 2))
            }
            "u32" => {
                if data.len() < 4 {
                    anyhow::bail!("Not enough data for u32");
                }
                let value = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Ok((JsonValue::Number(value.into()), 4))
            }
            "u64" => {
                if data.len() < 8 {
                    anyhow::bail!("Not enough data for u64");
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[0..8]);
                let value = u64::from_le_bytes(bytes);
                Ok((JsonValue::Number(value.into()), 8))
            }
            "u128" => {
                if data.len() < 16 {
                    anyhow::bail!("Not enough data for u128");
                }
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&data[0..16]);
                let value = u128::from_le_bytes(bytes);
                Ok((JsonValue::String(value.to_string()), 16))
            }
            "i8" => {
                if data.len() < 1 {
                    anyhow::bail!("Not enough data for i8");
                }
                let value = i8::from_le_bytes([data[0]]);
                Ok((JsonValue::Number(value.into()), 1))
            }
            "i16" => {
                if data.len() < 2 {
                    anyhow::bail!("Not enough data for i16");
                }
                let value = i16::from_le_bytes([data[0], data[1]]);
                Ok((JsonValue::Number(value.into()), 2))
            }
            "i32" => {
                if data.len() < 4 {
                    anyhow::bail!("Not enough data for i32");
                }
                let value = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Ok((JsonValue::Number(value.into()), 4))
            }
            "i64" => {
                if data.len() < 8 {
                    anyhow::bail!("Not enough data for i64");
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[0..8]);
                let value = i64::from_le_bytes(bytes);
                Ok((JsonValue::Number(value.into()), 8))
            }
            "i128" => {
                if data.len() < 16 {
                    anyhow::bail!("Not enough data for i128");
                }
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&data[0..16]);
                let value = i128::from_le_bytes(bytes);
                Ok((JsonValue::String(value.to_string()), 16))
            }
            "bool" => {
                if data.len() < 1 {
                    anyhow::bail!("Not enough data for bool");
                }
                Ok((JsonValue::Bool(data[0] != 0), 1))
            }
            "string" => self.decode_string(data),
            "publicKey" | "pubkey" => self.decode_pubkey(data),
            _ => {
                debug!("Unknown primitive type: {}", type_name);
                Ok((JsonValue::String(format!("<unknown: {}>", type_name)), 0))
            }
        }
    }

    /// Decode string (length prefix + UTF-8 bytes)
    fn decode_string(&self, data: &[u8]) -> Result<(JsonValue, usize)> {
        if data.len() < 4 {
            anyhow::bail!("Not enough data for string length");
        }

        let length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + length {
            anyhow::bail!("Not enough data for string content");
        }

        let string = String::from_utf8(data[4..4 + length].to_vec()).context("Invalid UTF-8 in string")?;

        Ok((JsonValue::String(string), 4 + length))
    }

    /// Decode public key (32 bytes)
    fn decode_pubkey(&self, data: &[u8]) -> Result<(JsonValue, usize)> {
        if data.len() < 32 {
            anyhow::bail!("Not enough data for pubkey");
        }

        let pubkey_bytes = &data[0..32];
        let pubkey = bs58::encode(pubkey_bytes).into_string();

        Ok((JsonValue::String(pubkey), 32))
    }

    /// Decode Option<T>
    fn decode_option(&self, data: &[u8], inner_type: &IDLTypeReference) -> Result<(JsonValue, usize)> {
        if data.len() < 1 {
            anyhow::bail!("Not enough data for option discriminator");
        }

        if data[0] == 0 {
            // None
            Ok((JsonValue::Null, 1))
        } else {
            // Some(value)
            let (value, bytes_read) = self.decode_type(&data[1..], inner_type)?;
            Ok((value, 1 + bytes_read))
        }
    }

    /// Decode Vec<T>
    fn decode_vec(&self, data: &[u8], element_type: &IDLTypeReference) -> Result<(JsonValue, usize)> {
        if data.len() < 4 {
            anyhow::bail!("Not enough data for vec length");
        }

        let length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let mut elements = Vec::new();
        let mut offset = 4;

        for _ in 0..length {
            let (value, bytes_read) = self.decode_type(&data[offset..], element_type)?;
            elements.push(value);
            offset += bytes_read;
        }

        Ok((JsonValue::Array(elements), offset))
    }

    /// Decode Array<T, N>
    fn decode_array(&self, data: &[u8], array_info: &[Box<IDLTypeReference>; 2]) -> Result<(JsonValue, usize)> {
        // array_info[0] is the element type, array_info[1] is the length (as a type reference)
        // For now, we'll just decode as much as we can
        // TODO: Properly extract array length from type

        debug!("Array decoding not fully implemented yet");
        Ok((JsonValue::String("<array>".to_string()), 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_primitives() {
        let decoder = IdlDecoder { instruction_map: HashMap::new() };

        // Test u64
        let data = 42u64.to_le_bytes();
        let (value, size) = decoder.decode_primitive(&data, "u64").unwrap();
        assert_eq!(value, JsonValue::Number(42.into()));
        assert_eq!(size, 8);

        // Test bool
        let data = [1u8];
        let (value, size) = decoder.decode_primitive(&data, "bool").unwrap();
        assert_eq!(value, JsonValue::Bool(true));
        assert_eq!(size, 1);
    }

    #[test]
    fn test_decode_option() {
        let decoder = IdlDecoder { instruction_map: HashMap::new() };

        // Test None
        let data = [0u8];
        let (value, size) = decoder.decode_option(&data, &IDLTypeReference::Primitive("u64".to_string())).unwrap();
        assert_eq!(value, JsonValue::Null);
        assert_eq!(size, 1);

        // Test Some(42)
        let mut data = vec![1u8];
        data.extend_from_slice(&42u64.to_le_bytes());
        let (value, size) = decoder.decode_option(&data, &IDLTypeReference::Primitive("u64".to_string())).unwrap();
        assert_eq!(value, JsonValue::Number(42.into()));
        assert_eq!(size, 9);
    }
}
