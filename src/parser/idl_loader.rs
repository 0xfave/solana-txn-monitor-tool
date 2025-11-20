use crate::types::{IDLInstruction, IDLSchema};
use anyhow::{Context, Result};
use std::path::Path;

/// Load IDL from a file path
pub fn load_idl_from_file<P: AsRef<Path>>(path: P) -> Result<IDLSchema> {
    let path = path.as_ref();
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read IDL file: {}", path.display()))?;

    load_idl_from_string(&content).with_context(|| format!("Failed to parse IDL from file: {}", path.display()))
}

/// Load IDL from a JSON string
pub fn load_idl_from_string(json: &str) -> Result<IDLSchema> {
    let mut idl: IDLSchema = serde_json::from_str(json).context("Failed to deserialize IDL JSON")?;

    normalize_idl(&mut idl)?;
    validate_idl(&idl)?;
    Ok(idl)
}

/// Normalize IDL to handle both old and new Anchor formats
fn normalize_idl(idl: &mut IDLSchema) -> Result<()> {
    // Handle new Anchor 0.30+ format with metadata object
    if let Some(ref metadata) = idl.metadata {
        if idl.name.is_empty() {
            idl.name = metadata.name.clone();
        }
        if idl.version.is_empty() {
            idl.version = metadata.version.clone();
        }
    }
    Ok(())
}

/// Validate IDL structure
fn validate_idl(idl: &IDLSchema) -> Result<()> {
    anyhow::ensure!(!idl.name.is_empty(), "IDL must have a program name");
    anyhow::ensure!(!idl.instructions.is_empty(), "IDL must have at least one instruction");

    // Validate each instruction has a name
    for (idx, instruction) in idl.instructions.iter().enumerate() {
        anyhow::ensure!(!instruction.name.is_empty(), "Instruction at index {} must have a name", idx);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_minimal_idl() -> Result<()> {
        let idl_json = r#"{
            "version": "0.1.0",
            "name": "test_program",
            "instructions": [
                {
                    "name": "initialize",
                    "accounts": [
                        {
                            "name": "authority",
                            "isMut": false,
                            "isSigner": true
                        }
                    ],
                    "args": []
                }
            ]
        }"#;

        let idl = load_idl_from_string(idl_json)?;
        assert_eq!(idl.name, "test_program");
        assert_eq!(idl.instructions.len(), 1);
        assert_eq!(idl.instructions[0].name, "initialize");
        Ok(())
    }

    #[test]
    fn test_load_idl_with_args() -> Result<()> {
        let idl_json = r#"{
            "version": "0.1.0",
            "name": "swap_program",
            "instructions": [
                {
                    "name": "swap",
                    "accounts": [
                        {
                            "name": "userAuthority",
                            "isMut": false,
                            "isSigner": true
                        },
                        {
                            "name": "tokenAccount",
                            "isMut": true,
                            "isSigner": false
                        }
                    ],
                    "args": [
                        {
                            "name": "amountIn",
                            "type": "u64"
                        },
                        {
                            "name": "minimumAmountOut",
                            "type": "u64"
                        }
                    ]
                }
            ]
        }"#;

        let idl = load_idl_from_string(idl_json)?;
        assert_eq!(idl.name, "swap_program");
        assert_eq!(idl.instructions[0].args.len(), 2);
        assert_eq!(idl.instructions[0].args[0].name, "amountIn");
        Ok(())
    }

    #[test]
    fn test_validate_empty_name() {
        let idl_json = r#"{
            "version": "0.1.0",
            "name": "",
            "instructions": []
        }"#;

        let result = load_idl_from_string(idl_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("program name"));
    }

    #[test]
    fn test_validate_no_instructions() {
        let idl_json = r#"{
            "version": "0.1.0",
            "name": "test_program",
            "instructions": []
        }"#;

        let result = load_idl_from_string(idl_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one instruction"));
    }

    #[test]
    fn test_invalid_json() {
        let idl_json = r#"{ invalid json }"#;
        let result = load_idl_from_string(idl_json);
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // Only run when idls/raydium_cp_swap.json exists
    fn test_load_real_raydium_idl() -> Result<()> {
        // Note: Raydium uses Anchor 0.30+ format which has some complex nested types
        // This test validates we can at least parse the basic structure
        let content = std::fs::read_to_string("idls/raydium_cp_swap.json")?;

        // Parse as generic JSON first to verify file is valid
        let json: serde_json::Value = serde_json::from_str(&content)?;

        // Check basic structure
        assert!(json.get("address").is_some());
        assert!(json.get("metadata").is_some());
        assert!(json.get("instructions").is_some());

        let address = json["address"].as_str().unwrap();
        assert_eq!(address, "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");

        let metadata = &json["metadata"];
        assert_eq!(metadata["name"].as_str().unwrap(), "raydium_cp_swap");
        assert_eq!(metadata["version"].as_str().unwrap(), "0.2.0");

        // Check instructions array
        let instructions = json["instructions"].as_array().unwrap();
        assert!(!instructions.is_empty());

        let instruction_names: Vec<_> = instructions.iter().map(|i| i["name"].as_str().unwrap()).collect();

        assert!(instruction_names.contains(&"swap_base_input"));
        assert!(instruction_names.contains(&"swap_base_output"));
        assert!(instruction_names.contains(&"initialize"));

        // Find swap_base_input and check discriminator
        let swap_inst = instructions.iter().find(|i| i["name"].as_str().unwrap() == "swap_base_input").unwrap();

        let disc = swap_inst["discriminator"].as_array().unwrap();
        assert_eq!(disc.len(), 8);
        assert_eq!(disc[0].as_u64().unwrap(), 143);

        println!("✅ Successfully validated Raydium CPMM IDL structure");
        println!("   Address: {}", address);
        println!("   Name: {}", metadata["name"].as_str().unwrap());
        println!("   Instructions: {}", instructions.len());

        Ok(())
    }

    #[test]
    #[ignore] // Only run when idls/jupiter_v6.json exists
    fn test_load_jupiter_v6_idl() -> Result<()> {
        // Load the Jupiter v6 aggregator IDL
        let content = std::fs::read_to_string("idls/jupiter_v6.json")?;

        // Parse as generic JSON first to verify file is valid
        let json: serde_json::Value = serde_json::from_str(&content)?;

        // Check basic structure
        assert!(json.get("address").is_some());
        assert!(json.get("metadata").is_some());
        assert!(json.get("instructions").is_some());

        let address = json["address"].as_str().unwrap();
        assert_eq!(address, "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

        let metadata = &json["metadata"];
        assert_eq!(metadata["name"].as_str().unwrap(), "jupiter");
        assert_eq!(metadata["version"].as_str().unwrap(), "0.1.0");

        // Check instructions array
        let instructions = json["instructions"].as_array().unwrap();
        assert!(!instructions.is_empty());

        // Verify route instruction
        let route = instructions
            .iter()
            .find(|i| i["name"].as_str().unwrap() == "route")
            .expect("Should have route instruction");

        let disc = route["discriminator"].as_array().unwrap();
        assert_eq!(disc.len(), 8);
        // route discriminator: [229, 23, 203, 151, 122, 227, 173, 42]
        assert_eq!(disc[0].as_u64().unwrap(), 229);
        assert_eq!(disc[1].as_u64().unwrap(), 23);

        // Check args
        let args = route["args"].as_array().unwrap();
        let arg_names: Vec<_> = args.iter().map(|a| a["name"].as_str().unwrap()).collect();

        assert!(arg_names.contains(&"in_amount"));
        assert!(arg_names.contains(&"quoted_out_amount"));
        assert!(arg_names.contains(&"route_plan"));

        println!("✅ Successfully validated Jupiter v6 IDL structure");
        println!("   Address: {}", address);
        println!("   Name: {}", metadata["name"].as_str().unwrap());
        println!("   Instructions: {}", instructions.len());
        println!("   route discriminator: {:?}", disc);

        // Verify discriminator calculation matches
        let route_inst = IDLInstruction {
            name: "route".to_string(),
            docs: vec![],
            discriminator: vec![],
            accounts: vec![],
            args: vec![],
            returns: None,
        };
        let computed_disc = route_inst.compute_discriminator();
        println!("   computed discriminator: {:?}", computed_disc);

        // Compare computed vs provided
        let provided_disc: Vec<u8> = disc.iter().map(|v| v.as_u64().unwrap() as u8).collect();
        assert_eq!(&computed_disc[..], &provided_disc[..], "Discriminator mismatch");

        Ok(())
    }
}
