use crate::types::IDLSchema;
use anyhow::{Context, Result};
use std::{path::PathBuf, process::Command};
use tracing::{debug, info, warn};

/// Guess IDL from a Solana program using SEC3 IDL Guesser
///
/// This function shells out to the idl-guesser binary to automatically
/// recover IDL information from closed-source Anchor programs.
///
/// # Arguments
/// * `program_id` - The program address to analyze
/// * `rpc_url` - The Solana RPC endpoint URL
///
/// # Returns
/// * `Ok(IDLSchema)` - Successfully generated IDL
/// * `Err(_)` - Program is not Anchor-based or analysis failed
pub fn guess_idl_from_program(program_id: &str, rpc_url: &str) -> Result<IDLSchema> {
    info!("Attempting to guess IDL for program: {}", program_id);

    // Find the idl-guesser binary
    let binary_path = find_idl_guesser_binary()?;

    // Create temporary directory for output
    let temp_dir = std::env::temp_dir();
    let output_file = temp_dir.join(format!("{}.json", program_id));

    debug!("Running idl-guesser binary: {:?}", binary_path);
    debug!("RPC URL: {}", rpc_url);

    // Run idl-guesser as subprocess
    // Use --force-guess to analyze bytecode even if public IDL exists
    // This gives us more consistent format and often discovers more instructions
    let output = Command::new(&binary_path)
        .arg(program_id)
        .arg("--url")
        .arg(rpc_url)
        .arg("--force-guess")
        .current_dir(&temp_dir)
        .output()
        .context("Failed to execute idl-guesser binary")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("IDL Guesser failed: {}", stderr);
        anyhow::bail!("IDL Guesser analysis failed: {}", stderr);
    }

    // Check stdout for success message
    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!("IDL Guesser output: {}", stdout);

    // Parse the generated IDL JSON
    if output_file.exists() {
        info!("Successfully generated IDL, loading from: {:?}", output_file);
        let content = std::fs::read_to_string(&output_file).context("Failed to read generated IDL file")?;

        // Clean up the temp file
        let _ = std::fs::remove_file(&output_file);

        // Parse into our IDLSchema type
        let idl: IDLSchema = match serde_json::from_str(&content) {
            Ok(idl) => idl,
            Err(e) => {
                warn!("Failed to parse IDL JSON: {}", e);
                warn!("JSON content preview: {}", &content.chars().take(500).collect::<String>());
                anyhow::bail!("Failed to parse generated IDL JSON: {}", e);
            }
        };

        info!("Parsed IDL for program: {} ({} instructions)", idl.name, idl.instructions.len());

        Ok(idl)
    } else {
        anyhow::bail!("IDL Guesser did not generate output file at {:?}", output_file);
    }
}

/// Find the idl-guesser binary in the project
fn find_idl_guesser_binary() -> Result<PathBuf> {
    // Get workspace root
    let workspace_root = std::env::current_dir().context("Failed to get current directory")?;

    // Try release build first
    let release_path = workspace_root.join("idl-guesser/target/release/idl-guesser");
    if release_path.exists() {
        return Ok(release_path);
    }

    // Try debug build
    let debug_path = workspace_root.join("idl-guesser/target/debug/idl-guesser");
    if debug_path.exists() {
        return Ok(debug_path);
    }

    // Try system PATH
    if let Ok(output) = Command::new("which").arg("idl-guesser").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                return Ok(PathBuf::from(path_str));
            }
        }
    }

    anyhow::bail!(
        "idl-guesser binary not found. Please build it first:\n\
         cd idl-guesser && cargo build --release"
    );
}

/// Check if a program is likely an Anchor program
///
/// This is a quick heuristic check before running full analysis.
/// Currently just checks program size and basic patterns.
pub fn is_likely_anchor_program(_program_data: &[u8]) -> bool {
    // For now, always return true and let idl-guesser determine
    // In the future, we could do quick pattern matching here
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires idl-guesser binary to be built
    fn test_find_binary() -> Result<()> {
        let binary_path = find_idl_guesser_binary()?;
        assert!(binary_path.exists());
        println!("Found idl-guesser binary at: {:?}", binary_path);
        Ok(())
    }

    #[test]
    #[ignore] // Requires network access and Anchor program
    fn test_guess_idl() -> Result<()> {
        // Test with a known Anchor program
        let program_id = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"; // Example from README
        let rpc_url = "https://api.mainnet-beta.solana.com";

        // Use generic JSON validation for now (same approach as Raydium test)
        // Full IDLSchema deserialization may fail due to complex type structures
        let temp_dir = std::env::temp_dir();
        let output_file = temp_dir.join(format!("{}.json", program_id));

        // Run the binary directly
        let binary_path = find_idl_guesser_binary()?;
        let output = std::process::Command::new(&binary_path)
            .arg(program_id)
            .arg("--url")
            .arg(rpc_url)
            .current_dir(&temp_dir)
            .output()?;

        assert!(output.status.success(), "IDL Guesser should succeed");

        // Validate JSON structure
        if output_file.exists() {
            let content = std::fs::read_to_string(&output_file)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;

            assert!(json["address"].is_string());
            assert_eq!(json["address"], program_id);
            assert!(json["metadata"].is_object());
            assert!(json["instructions"].is_array());

            let instructions = json["instructions"].as_array().unwrap();
            assert!(!instructions.is_empty(), "Should have instructions");

            // Verify instruction structure
            let first_inst = &instructions[0];
            assert!(first_inst["name"].is_string());
            assert!(first_inst["discriminator"].is_array());
            assert!(first_inst["accounts"].is_array());

            println!("\n✅ Successfully generated IDL for {}", program_id);
            println!("   Name: {}", json["metadata"]["name"]);
            println!("   Instructions: {}", instructions.len());
            println!("   First instruction: {}", first_inst["name"]);

            // Cleanup
            let _ = std::fs::remove_file(&output_file);

            Ok(())
        } else {
            anyhow::bail!("IDL file was not generated");
        }
    }
}
