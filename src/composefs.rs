use std::process::Command;
use anyhow::{anyhow, Result, Context};

pub fn pull_image(image_ref: &str) -> Result<String> {
    let final_ref = if image_ref.contains("://") {
        image_ref.to_string()
    } else {
        format!("docker://{}", image_ref)
    };

    let output = Command::new("bootc")
        .args(["internals", "cfs", "--system", "oci", "pull", &final_ref])
        .output()
        .context("failed to execute bootc internals cfs oci pull")?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("pull failed: {}", stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("manifest ") {
            let digest = trimmed["manifest ".len()..].trim().to_string();
            return Ok(digest);
        }
    }
    
    // Fallback if not found
    Ok(stdout.trim().to_string())
}

pub fn create_image(image_id: &str) -> Result<String> {
    let output = Command::new("bootc")
        .args(["internals", "cfs", "--system", "oci", "create-image", image_id])
        .output()
        .context("failed to execute bootc internals cfs oci create-image")?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("create-image failed: {}", stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().to_string())
}
