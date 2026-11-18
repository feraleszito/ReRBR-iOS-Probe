//! Read-only evidence from the scripts actually selected by the original boot resolver.
//! Presence of keys is a prerequisite, never a compatibility certification.
use rbr_formats::lsp::Node;
use std::io::Read;

/// Same archive overlay and original per-slot folder as the current boot backend.
/// This audit never extracts or writes private scripts.
pub(crate) fn inspect(path: &std::path::Path, folder: &str, surface: &str) -> Result<PhysicsAudit, String> {
    let mut zip = rbr_formats::rbz::open(path).map_err(|e| e.to_string())?;
    let mut scripts = Vec::new();
    for name in [
        "Physics/physics.lsp".into(),
        format!("{folder}common.lsp"),
        format!("{folder}{surface}.lsp"),
        format!("{folder}setups/d_{surface}.lsp"),
        format!("{folder}r_{surface}.lsp"),
    ] {
        let mut text = String::new();
        rbr_formats::rbz::by_name(&mut zip, &name)
            .map_err(|e| format!("physics audit {name}: {e}"))?
            .read_to_string(&mut text)
            .map_err(|e| format!("physics audit {name}: {e}"))?;
        scripts.push((name, rbr_formats::lsp::parse(&text).map_err(|e| e.to_string())?));
    }
    Ok(PhysicsAudit::inspect(&scripts))
}

/// Audits an NGP car pack's real `.lsp` scripts directly (`common.lsp`, `<surface>.lsp`, etc.).
pub(crate) fn inspect_pack(
    archive: &std::path::Path,
    pack: &rbr_formats::car_pack::Pack,
    surface: &str,
) -> Result<PhysicsAudit, String> {
    let mut zip = rbr_formats::rbz::open(archive).map_err(|e| e.to_string())?;
    let mut physics_text = String::new();
    rbr_formats::rbz::by_name(&mut zip, "Physics/physics.lsp")
        .map_err(|e| format!("physics audit Physics/physics.lsp: {e}"))?
        .read_to_string(&mut physics_text)
        .map_err(|e| format!("physics audit Physics/physics.lsp: {e}"))?;
    let mut scripts =
        vec![("Physics/physics.lsp".into(), rbr_formats::lsp::parse(&physics_text).map_err(|e| e.to_string())?)];
    for rel in
        ["common.lsp".into(), format!("{surface}.lsp"), format!("setups/d_{surface}.lsp"), format!("r_{surface}.lsp")]
    {
        let bytes = pack.read(&rel)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("physics audit {rel}: {e}"))?;
        scripts.push((rel, rbr_formats::lsp::parse(text).map_err(|e| e.to_string())?));
    }
    Ok(PhysicsAudit::inspect(&scripts))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhysicsAudit {
    pub scripts: Vec<String>,
    pub missing_ngp_keys: Vec<String>,
    pub car_id: Option<u32>,
    pub features_ngp: Option<u32>,
    pub max_torque_ngp: Option<f32>,
}
impl PhysicsAudit {
    pub(crate) fn inspect(scripts: &[(String, Vec<Node>)]) -> Self {
        let mut required = vec!["MaxTorque_NGP".to_string(), "Features_NGP".to_string()];
        for index in 0..16 {
            required.push(format!("Tq_{index:02}_RPM_NGP"));
            required.push(format!("Tq_{index:02}_NGP"));
        }
        let missing_ngp_keys =
            required.into_iter().filter(|key| !scripts.iter().any(|(_, nodes)| has_numeric_key(nodes, key))).collect();
        let car_id = scripts.iter().rev().find_map(|(_, n)| numeric_key(n, "CarId")).map(|v| v.round() as u32);
        let features_ngp =
            scripts.iter().rev().find_map(|(_, n)| numeric_key(n, "Features_NGP")).map(|v| v.round() as u32);
        let max_torque_ngp = scripts.iter().rev().find_map(|(_, n)| numeric_key(n, "MaxTorque_NGP"));
        Self {
            scripts: scripts.iter().map(|(name, _)| name.clone()).collect(),
            missing_ngp_keys,
            car_id,
            features_ngp,
            max_torque_ngp,
        }
    }

    pub fn summary(&self) -> String {
        if self.missing_ngp_keys.is_empty() {
            match (self.car_id, self.max_torque_ngp, self.features_ngp) {
                (Some(id), Some(tq), feat) => {
                    format!("NGP PARSED (CarId={id} MaxTq={tq:.0}Nm Feat={}) / CRC UNVERIFIED", feat.unwrap_or(0))
                }
                _ => "NGP KEYS PRESENT / CRC AND PHYSICS PARITY UNVERIFIED".into(),
            }
        } else {
            format!("UNVERIFIED PHYSICS: {} NGP KEYS MISSING / CRC UNCHECKED", self.missing_ngp_keys.len())
        }
    }
}

fn numeric_key(nodes: &[Node], key: &str) -> Option<f32> {
    nodes
        .windows(2)
        .find_map(|pair| match pair {
            [Node::Atom(k), Node::Atom(v)] if k == key => v.parse::<f32>().ok().filter(|x| x.is_finite()),
            _ => None,
        })
        .or_else(|| {
            nodes.iter().find_map(|node| match node {
                Node::List(children) => numeric_key(children, key),
                _ => None,
            })
        })
}

fn has_numeric_key(nodes: &[Node], key: &str) -> bool {
    numeric_key(nodes, key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_non_numeric_values_do_not_satisfy_prerequisites() {
        let nodes =
            rbr_formats::lsp::parse("; MaxTorque_NGP 400\n(\"Root\" MaxTorque_NGP NaN Features_NGP 6)").unwrap();
        let audit = PhysicsAudit::inspect(&[("selected/options.lsp".into(), nodes)]);
        assert!(audit.missing_ngp_keys.iter().any(|key| key == "MaxTorque_NGP"));
        assert!(!audit.missing_ngp_keys.iter().any(|key| key == "Features_NGP"));
        assert_eq!(audit.scripts, ["selected/options.lsp"]);
        assert!(audit.summary().contains("UNVERIFIED"));
    }
}
