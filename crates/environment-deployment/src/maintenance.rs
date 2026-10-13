use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, ResolvedEnvironment, Result, desired_digest};

/// A closed weekly window. Only the currently supported UTC and Tokyo zones are admitted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MaintenanceWindow {
    pub weekday: String,
    pub start_minutes: u16,
    pub end_minutes: u16,
    pub timezone: String,
}

impl MaintenanceWindow {
    /// Parses `Sun 02:00-03:00 Asia/Tokyo` without interpreting arbitrary systemd syntax.
    ///
    /// # Errors
    /// Rejects missing, ambiguous, overnight and unsupported windows.
    pub fn parse(value: &str) -> Result<Self> {
        let parts = value.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 3
            || !["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].contains(&parts[0])
            || !["UTC", "Asia/Tokyo"].contains(&parts[2])
        {
            return Err(Error::new(
                "invalid maintenance window or unsupported timezone",
            ));
        }
        let (start, end) = parts[1]
            .split_once('-')
            .ok_or_else(|| Error::new("missing maintenance range"))?;
        let start_minutes = clock(start)?;
        let end_minutes = clock(end)?;
        if start_minutes >= end_minutes {
            return Err(Error::new(
                "maintenance window must end on the same day after its start",
            ));
        }
        Ok(Self {
            weekday: parts[0].into(),
            start_minutes,
            end_minutes,
            timezone: parts[2].into(),
        })
    }
}

fn clock(value: &str) -> Result<u16> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 2 || b.is_ascii_digit())
    {
        return Err(Error::new("maintenance time must be HH:MM"));
    }
    let hour = value[..2]
        .parse::<u16>()
        .map_err(|_| Error::new("invalid maintenance hour"))?;
    let minute = value[3..]
        .parse::<u16>()
        .map_err(|_| Error::new("invalid maintenance minute"))?;
    if hour > 23 || minute > 59 {
        return Err(Error::new("invalid maintenance clock"));
    }
    Ok(hour * 60 + minute)
}

/// Owner-reviewed non-secret binding; this does not itself authorize maintenance execution.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceBinding {
    pub environment_id: String,
    pub desired_digest: String,
    pub inventory_path: String,
    pub inventory_digest: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MaintenanceTimer {
    pub schema: &'static str,
    pub binding: MaintenanceBinding,
    pub window: MaintenanceWindow,
    pub jitter_seconds: u32,
    pub unit: String,
    pub execution_authorized: bool,
}

/// Renders a review-only timer after attesting the environment, configuration and inventory.
/// No timer is installed or enabled and approval semantics remain a separate owner decision.
///
/// # Errors
/// Rejects identity/digest mismatches and jitter that can cross the declared end boundary.
pub fn render_maintenance_timer(
    desired: &ResolvedEnvironment,
    binding: &MaintenanceBinding,
    inventory: &[u8],
    jitter_seconds: u32,
) -> Result<MaintenanceTimer> {
    let id = &desired.environment.environment_id;
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(Error::new("invalid maintenance environment identity"));
    }
    if binding.environment_id != *id
        || binding.desired_digest != desired_digest(desired)?
        || binding.inventory_path != format!("/etc/vpremises/inventory/{id}.ini")
        || binding.inventory_digest != format!("{:x}", Sha256::digest(inventory))
    {
        return Err(Error::new(
            "maintenance environment, configuration or inventory mismatch",
        ));
    }
    let window = MaintenanceWindow::parse(&desired.environment.maintenance.window)?;
    if jitter_seconds >= u32::from(window.end_minutes - window.start_minutes) * 60 {
        return Err(Error::new(
            "maintenance jitter can cross the declared window",
        ));
    }
    let unit = format!(
        "[Unit]\nDescription=Review-only maintenance for {id}\n\n[Timer]\nOnCalendar={} *-*-* {:02}:{:02}:00 {}\nRandomizedDelaySec={jitter_seconds}s\nAccuracySec=1us\nPersistent=false\nUnit=vpremises-maintenance@{id}.service\n",
        window.weekday,
        window.start_minutes / 60,
        window.start_minutes % 60,
        window.timezone
    );
    Ok(MaintenanceTimer {
        schema: "vpremises://maintenance/timer-review/v1",
        binding: binding.clone(),
        window,
        jitter_seconds,
        unit,
        execution_authorized: false,
    })
}
