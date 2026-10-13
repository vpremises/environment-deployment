use serde_json::{Value, json};

use crate::{ResolvedEnvironment, Result, validate};

/// Renders a restricted project, profile, and instance as `OpenTofu` JSON.
///
/// # Errors
///
/// Rejects invalid placement, unsupported instance types, and unsafe limits.
pub fn render(resolved: &ResolvedEnvironment) -> Result<Value> {
    validate(resolved)?;
    let environment = &resolved.environment;
    let placement = environment
        .incus
        .as_ref()
        .ok_or("missing validated Incus placement")?;
    let instance_type = match &environment.instance_type {
        vpremises_environment_deployment::InstanceType::VirtualMachine => "virtual-machine",
        vpremises_environment_deployment::InstanceType::SystemContainer => "container",
        vpremises_environment_deployment::InstanceType::Native => {
            return Err("Incus supports virtual-machine or system-container".into());
        }
    };
    let profile_name = format!("{}-runtime", placement.project);
    let profile_ref = "${incus_profile.managed.name}";
    let project_ref = "${incus_project.managed.name}";
    Ok(json!({
      "terraform": {
        "required_providers": {
          "incus": { "source": "lxc/incus" }
        }
      },
      "resource": {
        "incus_project": {
          "managed": {
            "name": placement.project,
            "description": format!("vPremises environment {}", environment.environment_id),
            "force_destroy": false,
            "config": {
              "features.images": "true",
              "features.profiles": "true",
              "features.storage.volumes": "true",
              "restricted": "true",
              "restricted.backups": "allow"
            }
          }
        },
        "incus_profile": {
          "managed": {
            "name": profile_name,
            "project": project_ref,
            "description": "Managed by vPremises; do not edit manually",
            "config": {
              "limits.cpu": environment.resources.cpu.to_string(),
              "limits.memory": format!("{}MiB", environment.resources.memory_mib),
              "security.nesting": "false",
              "security.privileged": "false"
            },
            "device": [
              {
                "name": "root",
                "type": "disk",
                "properties": {
                  "path": "/",
                  "pool": placement.storage_pool,
                  "size": format!("{}GiB", environment.resources.root_disk_gib)
                }
              },
              {
                "name": "eth0",
                "type": "nic",
                "properties": {
                  "name": "eth0",
                  "network": placement.network
                }
              }
            ]
          }
        },
        "incus_instance": {
          "managed": {
            "name": placement.instance_name,
            "project": project_ref,
            "image": placement.image,
            "type": instance_type,
            "profiles": [profile_ref],
            "running": true,
            "ephemeral": false,
            "config": {
              "boot.autostart": "true",
              "boot.host_shutdown_action": "stop",
              "cloud-init.user-data": cloud_init(&resolved.service.process.user)
            },
            "wait_for": [{ "type": "cloud-init" }]
          }
        }
      }
    }))
}

fn cloud_init(user: &str) -> String {
    // JSON is a YAML-compatible structured document; input never becomes YAML syntax.
    let config = json!({
        "package_update": false,
        "package_upgrade": false,
        "users": [{ "name": user, "lock_passwd": true, "shell": "/usr/sbin/nologin" }]
    });
    format!("#cloud-config\n{config}\n")
}
